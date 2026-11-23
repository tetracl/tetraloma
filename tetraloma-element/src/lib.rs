// Design: section 26 ("Finite elements"), Listing E1; ADR-0071, ADR-0072.
//! Finite elements in Ciarlet's sense on the reference cells of tetraloma.
//!
//! This crate computes finite-element definitions on the host in `f64`, with no device
//! dependency. Elements are identified by an [`ElementKey`] and looked up through
//! [`Element::get`], which caches each element process-wide.
//!
//! The chapter [Finite elements] introduces the families, degree ranges, variants and DOF
//! layouts of this crate.
//!
//! # Example
//!
//! ```
//! use tetraloma_dims::CellType;
//! use tetraloma_element::{Element, ElementKey, Family, MapType, Variant};
//! use tetraloma_refcell::NodeFamily;
//!
//! let el = Element::get(ElementKey {
//!     family: Family::Lagrange, cell: CellType::Triangle, degree: 2, variant: Variant::Nodal(NodeFamily::Gll),
//! })?;
//! assert_eq!(el.dim(), 6);
//! assert_eq!(el.map_type(), MapType::Identity);
//! assert_eq!(el.entity_closure_dofs(1, 0), &[0, 1, 3]);
//! # Ok::<(), tetraloma_element::ElementError>(())
//! ```
//!
//! [Finite elements]: https://tetracl.github.io/tetraloma/fem/elements.html

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use tetraloma_dims::CellType;
use tetraloma_refcell::NodeFamily;

mod error;
mod layout;
mod tensor;

pub use error::ElementError;
pub use tensor::{Factor1d, TensorFactors};

// Design: section 26 ("Finite elements"), Listing E1.
/// A finite-element family on the reference cells of tetraloma.
///
/// One value names the family across every cell type. On quadrilaterals and hexahedra,
/// `N1curl` and `RaviartThomas` are the tensor-product spaces (DefElement's RTCE, RTCF, NCE
/// and NCF). See [Families and degrees] for the cells and degree ranges each family supports.
///
/// [Families and degrees]: https://tetracl.github.io/tetraloma/fem/elements.html#families-and-degrees
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    /// Scalar Lagrange spaces (continuous H¹ and discontinuous L²).
    Lagrange,
    /// First-kind Nédélec H(curl) spaces (RTCE on quadrilaterals, NCE on hexahedra).
    N1curl,
    /// Raviart–Thomas H(div) spaces (RTCF on quadrilaterals, NCF on hexahedra).
    RaviartThomas,
}

// Design: section 26 ("Finite elements"), Listing E1, "Variants".
/// The basis and DOF variant of a finite element.
///
/// See [Variants] for how the nodal and Legendre variants differ in their DOF layouts and
/// conditioning.
///
/// [Variants]: https://tetracl.github.io/tetraloma/fem/elements.html#variants
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Variant {
    /// Point evaluations at a `tetraloma-refcell` point family (`Equispaced` or `Gll`).
    ///
    /// At degree 0 there is one interior DOF, the value at the cell's midpoint, and the element
    /// is discontinuous only. `Nodal(NodeFamily::Gll)` is not offered on pyramids.
    Nodal(NodeFamily),
    /// Integral moments against an orthonormal basis.
    ///
    /// For [`Family::Lagrange`], this gives an L²-orthonormal basis with all DOFs in the cell
    /// interior (discontinuous only). For [`Family::N1curl`] and [`Family::RaviartThomas`], this
    /// is the Legendre-moment variant on every sub-entity.
    Legendre,
}

// Design: section 26 ("Finite elements"), Listing E1, "Map types".
/// How values and derivatives of a finite element map between the reference cell and a physical
/// cell.
///
/// See [Elements and DOFs] for the push-forward and pull-back formulas of each map type.
///
/// [Elements and DOFs]: https://tetracl.github.io/tetraloma/fem/elements.html#elements-and-dofs
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapType {
    /// Scalar push-forward: values copy unchanged (`u(x) = û(ξ)`). Used by Lagrange elements.
    Identity,
    /// Covariant Piola map (`u = Kᵀ û`), preserving tangential traces for H(curl) elements.
    CovariantPiola,
    /// Contravariant Piola map (`u = J û / det J`), preserving normal traces for H(div) elements.
    ContravariantPiola,
    /// L² Piola map (`u = û / det J`), used by discontinuous elements in exact sequences.
    L2Piola,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransformationKind {
    Identity,
    SignedPermutation,
    Dense,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Generator {
    Reversal,
    Rotation,
    Reflection,
}

// Design: section 26 ("Finite elements"), Listing E1.
/// The key that uniquely identifies a reference finite element in [`Element::get`].
///
/// `degree` uses the polyset-superdegree convention: degree 1 is the lowest-order continuous
/// Lagrange element and degree 0 is the lowest-order discontinuous (piecewise-constant) element.
/// See [Families and degrees].
///
/// [Families and degrees]: https://tetracl.github.io/tetraloma/fem/elements.html#families-and-degrees
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementKey {
    /// The finite-element family.
    pub family: Family,
    /// The reference cell type.
    pub cell: CellType,
    /// The polyset superdegree.
    pub degree: u32,
    /// The basis and DOF variant.
    pub variant: Variant,
}

/// Process-wide cache of constructed elements, keyed by [`ElementKey`].
static ELEMENT_CACHE: OnceLock<RwLock<HashMap<ElementKey, Arc<Element>>>> = OnceLock::new();

fn element_cache() -> &'static RwLock<HashMap<ElementKey, Arc<Element>>> {
    ELEMENT_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn get_dofs<'a>(
    table: &'a [Vec<Vec<usize>>],
    cell: CellType,
    method: &str,
    d: usize,
    e: usize,
) -> &'a [usize] {
    let entities = table.get(d).unwrap_or_else(|| {
        panic!(
            "{method}: {cell:?} has no sub-entities of dimension {d} (topological dimension is {})",
            table.len() - 1,
        )
    });
    entities.get(e).unwrap_or_else(|| {
        panic!(
            "{method}: {cell:?} has {} sub-entities of dimension {d}, not index {e}",
            entities.len(),
        )
    })
}

// Design: section 26 ("Finite elements"), Listing E1.
/// A Ciarlet finite element on one reference cell, in `f64`.
///
/// Obtain an element through [`Element::get`]. Each element is built once per [`ElementKey`]
/// and stored in a process-wide thread-safe cache, so repeated lookups return a handle to the
/// same allocation. See [Elements and DOFs].
///
/// # Example
///
/// ```
/// use tetraloma_dims::CellType;
/// use tetraloma_element::{Element, ElementKey, Family, MapType, Variant};
/// use tetraloma_refcell::NodeFamily;
///
/// let el = Element::get(ElementKey {
///     family: Family::Lagrange, cell: CellType::Tetrahedron, degree: 2, variant: Variant::Nodal(NodeFamily::Equispaced),
/// })?;
/// assert_eq!((el.dim(), el.reference_value_size(), el.map_type()), (10, 1, MapType::Identity));
/// assert_eq!((el.entity_dofs(0, 0), el.entity_dofs(1, 0)), (&[0][..], &[4][..]));
/// # Ok::<(), tetraloma_element::ElementError>(())
/// ```
///
/// [Elements and DOFs]: https://tetracl.github.io/tetraloma/fem/elements.html#elements-and-dofs
#[derive(Debug)]
pub struct Element {
    key: ElementKey,
    map_type: MapType,
    reference_value_size: usize,
    dim: usize,
    discontinuous_only: bool,
    embedded_superdegree: u32,
    entity_dofs: Vec<Vec<Vec<usize>>>,
    entity_closure_dofs: Vec<Vec<Vec<usize>>>,
}

impl Element {
    // Design: section 26 ("Finite elements"), Listing E1.
    /// Returns the finite element for `key`, building and caching it on first use.
    ///
    /// Repeated calls with the same `key` return clones of the same [`Arc<Element>`] from a
    /// process-wide thread-safe cache. See [Families and degrees].
    ///
    /// # Errors
    ///
    /// Returns [`ElementError::Unsupported`] if `key` is outside the supported table.
    ///
    /// # Example
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tetraloma_dims::CellType;
    /// use tetraloma_element::{Element, ElementKey, Family, Variant};
    /// use tetraloma_refcell::NodeFamily;
    ///
    /// let key = ElementKey {
    ///     family: Family::Lagrange, cell: CellType::Quadrilateral, degree: 3, variant: Variant::Nodal(NodeFamily::Gll),
    /// };
    /// let (a, b) = (Element::get(key)?, Element::get(key)?);
    /// assert!(Arc::ptr_eq(&a, &b) && a.dim() == 16);
    /// # Ok::<(), tetraloma_element::ElementError>(())
    /// ```
    ///
    /// [Families and degrees]: https://tetracl.github.io/tetraloma/fem/elements.html#families-and-degrees
    pub fn get(key: ElementKey) -> Result<Arc<Element>, ElementError> {
        layout::validate_key(key)?;
        let cache = element_cache();
        if let Some(el) = cache
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .cloned()
        {
            return Ok(el);
        }
        // Build outside the write lock so concurrent reads proceed and recursive lookups (for
        // tensor-product 1D factors) cannot deadlock.
        let built = Arc::new(Self::build(key));
        let mut guard = cache.write().unwrap_or_else(|e| e.into_inner());
        Ok(guard.entry(key).or_insert(built).clone())
    }

    fn build(key: ElementKey) -> Self {
        let dofs = layout::build_lagrange_layout(key);
        Self {
            key,
            map_type: MapType::Identity,
            reference_value_size: 1,
            dim: dofs.dim,
            discontinuous_only: dofs.discontinuous_only,
            embedded_superdegree: key.degree,
            entity_dofs: dofs.entity_dofs,
            entity_closure_dofs: dofs.entity_closure_dofs,
        }
    }

    // Design: section 26 ("Finite elements"), Listing E1.
    /// The [`ElementKey`] this element was looked up with. See [Families and degrees].
    ///
    /// # Example
    ///
    /// ```
    /// use tetraloma_dims::CellType;
    /// use tetraloma_element::{Element, ElementKey, Family, Variant};
    ///
    /// let key = ElementKey { family: Family::Lagrange, cell: CellType::Interval, degree: 2, variant: Variant::Legendre };
    /// assert_eq!(Element::get(key)?.key(), key);
    /// # Ok::<(), tetraloma_element::ElementError>(())
    /// ```
    ///
    /// [Families and degrees]: https://tetracl.github.io/tetraloma/fem/elements.html#families-and-degrees
    pub fn key(&self) -> ElementKey {
        self.key
    }

    // Design: section 26 ("Finite elements"), Listing E1, "Map types".
    /// The push-forward and pull-back map type of this element ([`MapType::Identity`] for
    /// Lagrange). See [Elements and DOFs].
    ///
    /// # Example
    ///
    /// ```
    /// use tetraloma_dims::CellType;
    /// use tetraloma_element::{Element, ElementKey, Family, MapType, Variant};
    /// use tetraloma_refcell::NodeFamily;
    ///
    /// let el = Element::get(ElementKey {
    ///     family: Family::Lagrange, cell: CellType::Triangle, degree: 1, variant: Variant::Nodal(NodeFamily::Equispaced),
    /// })?;
    /// assert_eq!(el.map_type(), MapType::Identity);
    /// # Ok::<(), tetraloma_element::ElementError>(())
    /// ```
    ///
    /// [Elements and DOFs]: https://tetracl.github.io/tetraloma/fem/elements.html#elements-and-dofs
    pub fn map_type(&self) -> MapType {
        self.map_type
    }

    // Design: section 26 ("Finite elements"), Listing E1.
    /// The number of scalar components of a reference basis function: `1` for [`Family::Lagrange`],
    /// and the cell's topological dimension for [`Family::N1curl`] and [`Family::RaviartThomas`].
    /// See [Elements and DOFs].
    ///
    /// # Example
    ///
    /// ```
    /// use tetraloma_dims::CellType;
    /// use tetraloma_element::{Element, ElementKey, Family, Variant};
    /// use tetraloma_refcell::NodeFamily;
    ///
    /// let el = Element::get(ElementKey {
    ///     family: Family::Lagrange, cell: CellType::Hexahedron, degree: 2, variant: Variant::Nodal(NodeFamily::Gll),
    /// })?;
    /// assert_eq!(el.reference_value_size(), 1);
    /// # Ok::<(), tetraloma_element::ElementError>(())
    /// ```
    ///
    /// [Elements and DOFs]: https://tetracl.github.io/tetraloma/fem/elements.html#elements-and-dofs
    pub fn reference_value_size(&self) -> usize {
        self.reference_value_size
    }

    // Design: section 26 ("Finite elements"), Listing E1.
    /// The number of degrees of freedom (the dimension of the element's function space).
    /// See [Elements and DOFs].
    ///
    /// # Example
    ///
    /// ```
    /// use tetraloma_dims::CellType;
    /// use tetraloma_element::{Element, ElementKey, Family, Variant};
    /// use tetraloma_refcell::NodeFamily;
    ///
    /// let el = Element::get(ElementKey {
    ///     family: Family::Lagrange, cell: CellType::Prism, degree: 2, variant: Variant::Nodal(NodeFamily::Equispaced),
    /// })?;
    /// assert_eq!(el.dim(), 18);
    /// # Ok::<(), tetraloma_element::ElementError>(())
    /// ```
    ///
    /// [Elements and DOFs]: https://tetracl.github.io/tetraloma/fem/elements.html#elements-and-dofs
    pub fn dim(&self) -> usize {
        self.dim
    }

    // Design: section 26 ("Finite elements"), Listing E1.
    /// The local DOF indices associated with the interior of sub-entity `e` of dimension `d`,
    /// excluding its boundary. Across all `(d, e)`, these slices partition `0..self.dim()`.
    /// See [Elements and DOFs].
    ///
    /// # Panics
    ///
    /// Panics if `d` exceeds the cell's topological dimension or `e` is out of range for `d`.
    ///
    /// # Example
    ///
    /// ```
    /// use tetraloma_dims::CellType;
    /// use tetraloma_element::{Element, ElementKey, Family, Variant};
    /// use tetraloma_refcell::NodeFamily;
    ///
    /// let el = Element::get(ElementKey {
    ///     family: Family::Lagrange, cell: CellType::Triangle, degree: 3, variant: Variant::Nodal(NodeFamily::Equispaced),
    /// })?;
    /// assert_eq!((el.entity_dofs(0, 2), el.entity_dofs(1, 0), el.entity_dofs(2, 0)), (&[2][..], &[3, 4][..], &[9][..]));
    /// # Ok::<(), tetraloma_element::ElementError>(())
    /// ```
    ///
    /// [Elements and DOFs]: https://tetracl.github.io/tetraloma/fem/elements.html#elements-and-dofs
    pub fn entity_dofs(&self, d: usize, e: usize) -> &[usize] {
        get_dofs(&self.entity_dofs, self.key.cell, "entity_dofs", d, e)
    }

    // Design: section 26 ("Finite elements"), Listing E1.
    /// The local DOF indices on the closure of sub-entity `e` of dimension `d`: all DOFs
    /// belonging to `(d, e)` and every lower-dimensional sub-entity contained in it, in
    /// ascending dimension and sub-entity order. See [Elements and DOFs].
    ///
    /// # Panics
    ///
    /// Panics if `d` exceeds the cell's topological dimension or `e` is out of range for `d`.
    ///
    /// # Example
    ///
    /// ```
    /// use tetraloma_dims::CellType;
    /// use tetraloma_element::{Element, ElementKey, Family, Variant};
    /// use tetraloma_refcell::NodeFamily;
    ///
    /// let el = Element::get(ElementKey {
    ///     family: Family::Lagrange, cell: CellType::Triangle, degree: 3, variant: Variant::Nodal(NodeFamily::Equispaced),
    /// })?;
    /// assert_eq!(el.entity_closure_dofs(1, 2), &[1, 2, 7, 8]);
    /// # Ok::<(), tetraloma_element::ElementError>(())
    /// ```
    ///
    /// [Elements and DOFs]: https://tetracl.github.io/tetraloma/fem/elements.html#elements-and-dofs
    pub fn entity_closure_dofs(&self, d: usize, e: usize) -> &[usize] {
        get_dofs(
            &self.entity_closure_dofs,
            self.key.cell,
            "entity_closure_dofs",
            d,
            e,
        )
    }

    // Design: section 26 ("Finite elements"), Listing E1, "Variants"; ADR-0072.
    /// Returns `true` if every DOF lies in the cell interior (`Variant::Legendre` at every degree
    /// and nodal Lagrange at degree 0). See [Variants].
    ///
    /// # Example
    ///
    /// ```
    /// use tetraloma_dims::CellType;
    /// use tetraloma_element::{Element, ElementKey, Family, Variant};
    /// use tetraloma_refcell::NodeFamily;
    ///
    /// let p0 = Element::get(ElementKey {
    ///     family: Family::Lagrange, cell: CellType::Triangle, degree: 0, variant: Variant::Nodal(NodeFamily::Gll),
    /// })?;
    /// let p2 = Element::get(ElementKey {
    ///     family: Family::Lagrange, cell: CellType::Triangle, degree: 2, variant: Variant::Nodal(NodeFamily::Gll),
    /// })?;
    /// assert!(p0.discontinuous_only() && !p2.discontinuous_only());
    /// # Ok::<(), tetraloma_element::ElementError>(())
    /// ```
    ///
    /// [Variants]: https://tetracl.github.io/tetraloma/fem/elements.html#variants
    pub fn discontinuous_only(&self) -> bool {
        self.discontinuous_only
    }

    // Design: section 26 ("Finite elements"), Listing E1.
    /// The smallest Lagrange degree whose space contains this element's space (equals
    /// [`ElementKey::degree`] for Lagrange). See [Families and degrees].
    ///
    /// # Example
    ///
    /// ```
    /// use tetraloma_dims::CellType;
    /// use tetraloma_element::{Element, ElementKey, Family, Variant};
    /// use tetraloma_refcell::NodeFamily;
    ///
    /// let el = Element::get(ElementKey {
    ///     family: Family::Lagrange, cell: CellType::Pyramid, degree: 3, variant: Variant::Nodal(NodeFamily::Equispaced),
    /// })?;
    /// assert_eq!(el.embedded_superdegree(), 3);
    /// # Ok::<(), tetraloma_element::ElementError>(())
    /// ```
    ///
    /// [Families and degrees]: https://tetracl.github.io/tetraloma/fem/elements.html#families-and-degrees
    pub fn embedded_superdegree(&self) -> u32 {
        self.embedded_superdegree
    }

    #[doc(hidden)]
    pub fn coefficients(&self) -> &[f64] {
        unimplemented!("pending M11.T5")
    }

    #[doc(hidden)]
    pub fn tabulate(
        &self,
        points: &[f64],
        nderivs: u32,
    ) -> Result<ElementTabulation, ElementError> {
        let _ = (points, nderivs);
        unimplemented!("pending M11.T5")
    }

    #[doc(hidden)]
    pub fn interpolation_points(&self) -> &[f64] {
        unimplemented!("pending M11.T5")
    }

    #[doc(hidden)]
    pub fn interpolation_matrix(&self) -> &[f64] {
        unimplemented!("pending M11.T5")
    }

    #[doc(hidden)]
    pub fn transformation_kind(&self) -> TransformationKind {
        unimplemented!("pending M11.T6")
    }

    #[doc(hidden)]
    pub fn base_transformation(
        &self,
        entity: CellType,
        generator: Generator,
    ) -> Option<&BaseTransformation> {
        let _ = (entity, generator);
        unimplemented!("pending M11.T6")
    }

    #[doc(hidden)]
    pub fn tensor_factors(&self) -> Option<&TensorFactors> {
        unimplemented!("pending M11.T7")
    }
}

#[doc(hidden)]
pub struct ElementTabulation {
    pub nderivs: u32,
    pub num_points: usize,
    pub dim: usize,
    pub value_size: usize,
    pub values: Vec<f64>,
}

#[doc(hidden)]
pub enum BaseTransformation {
    SignedPermutation { perm: Vec<u32>, negate: Vec<bool> },
    Dense { n: usize, matrix: Vec<f64> },
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use tetraloma_dims::CellType;
    use tetraloma_refcell::{NodeFamily, RefCellError, num_lagrange_nodes, tdim};

    use super::*;

    const ALL_CELLS: [CellType; 7] = [
        CellType::Interval,
        CellType::Triangle,
        CellType::Quadrilateral,
        CellType::Tetrahedron,
        CellType::Hexahedron,
        CellType::Prism,
        CellType::Pyramid,
    ];

    fn lagrange(cell: CellType, degree: u32, variant: Variant) -> ElementKey {
        ElementKey {
            family: Family::Lagrange,
            cell,
            degree,
            variant,
        }
    }

    #[test]
    fn cache_returns_same_arc_across_threads() {
        let key = lagrange(CellType::Triangle, 4, Variant::Nodal(NodeFamily::Gll));
        let handles: Vec<_> = (0..8)
            .map(|_| std::thread::spawn(move || Element::get(key).unwrap()))
            .collect();
        let results: Vec<Arc<Element>> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert!(results[1..].iter().all(|el| Arc::ptr_eq(&results[0], el)));
    }

    #[test]
    fn lagrange_accessors_and_degree_zero_behaviour() {
        // Keep degrees <= 3 on 3D cells here so dense construction in M11.T5 stays fast
        // (docs/plans/M11.md: no test builds a hexahedron above p = 10 before T7).
        for cell in ALL_CELLS {
            let td = tdim(cell);
            let variants: &[Variant] = if cell == CellType::Pyramid {
                &[Variant::Nodal(NodeFamily::Equispaced), Variant::Legendre]
            } else {
                &[
                    Variant::Nodal(NodeFamily::Equispaced),
                    Variant::Nodal(NodeFamily::Gll),
                    Variant::Legendre,
                ]
            };
            for &variant in variants {
                for degree in 0..=3 {
                    let key = lagrange(cell, degree, variant);
                    let el = Element::get(key).unwrap();
                    assert_eq!(el.key(), key);
                    assert_eq!(el.map_type(), MapType::Identity);
                    assert_eq!(el.reference_value_size(), 1);
                    assert_eq!(el.dim(), num_lagrange_nodes(cell, degree));
                    assert_eq!(el.embedded_superdegree(), degree);
                    assert_eq!(
                        el.discontinuous_only(),
                        matches!(variant, Variant::Legendre) || degree == 0
                    );
                    if degree == 0 {
                        assert_eq!((el.dim(), el.entity_dofs(td, 0)), (1, &[0][..]));
                        assert_eq!(el.entity_closure_dofs(td, 0), &[0]);
                        for d in 0..td {
                            assert!(el.entity_dofs(d, 0).is_empty());
                            assert!(el.entity_closure_dofs(d, 0).is_empty());
                        }
                    }
                }
            }
        }
        for (cell, max_p) in [
            (CellType::Interval, 20),
            (CellType::Triangle, 15),
            (CellType::Pyramid, 8),
        ] {
            let el = Element::get(lagrange(
                cell,
                max_p,
                Variant::Nodal(NodeFamily::Equispaced),
            ))
            .unwrap();
            assert_eq!(el.dim(), num_lagrange_nodes(cell, max_p));
        }
    }

    #[test]
    fn error_display_source_and_from_refcell() {
        let unsupported = Element::get(ElementKey {
            family: Family::N1curl,
            cell: CellType::Triangle,
            degree: 1,
            variant: Variant::Legendre,
        })
        .unwrap_err();
        assert!(unsupported.to_string().contains("N1curl"));
        assert!(unsupported.source().is_none());

        let inner = RefCellError::UnsupportedDerivativeOrder(2);
        let wrapped: ElementError = inner.into();
        assert_eq!(wrapped, ElementError::RefCell(inner));
        assert_eq!(wrapped.to_string(), inner.to_string());
        assert!(wrapped.source().is_some());
    }

    #[test]
    #[should_panic(expected = "entity_dofs: Triangle has no sub-entities of dimension 3")]
    fn entity_dofs_panics_on_dim_out_of_range() {
        let el = Element::get(lagrange(
            CellType::Triangle,
            1,
            Variant::Nodal(NodeFamily::Equispaced),
        ))
        .unwrap();
        let _ = el.entity_dofs(3, 0);
    }

    #[test]
    #[should_panic(expected = "entity_dofs: Triangle has 3 sub-entities of dimension 1")]
    fn entity_dofs_panics_on_index_out_of_range() {
        let el = Element::get(lagrange(
            CellType::Triangle,
            1,
            Variant::Nodal(NodeFamily::Equispaced),
        ))
        .unwrap();
        let _ = el.entity_dofs(1, 3);
    }

    #[test]
    #[should_panic(expected = "entity_closure_dofs: Interval has no sub-entities of dimension 2")]
    fn entity_closure_dofs_panics_on_dim_out_of_range() {
        let el = Element::get(lagrange(
            CellType::Interval,
            1,
            Variant::Nodal(NodeFamily::Equispaced),
        ))
        .unwrap();
        let _ = el.entity_closure_dofs(2, 0);
    }

    #[test]
    #[should_panic(expected = "entity_closure_dofs: Interval has 2 sub-entities of dimension 0")]
    fn entity_closure_dofs_panics_on_index_out_of_range() {
        let el = Element::get(lagrange(
            CellType::Interval,
            1,
            Variant::Nodal(NodeFamily::Equispaced),
        ))
        .unwrap();
        let _ = el.entity_closure_dofs(0, 2);
    }
}
