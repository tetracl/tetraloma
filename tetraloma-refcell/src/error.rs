//! Errors of tetraloma-refcell.

use core::fmt;

use tetraloma_dims::CellType;

use crate::{NodeFamily, QuadratureType};

// tetraloma-refcell depends only on tetraloma-dims, so it cannot return `GridError`;
// tetraloma-geometry maps these errors onto `GridError` (docs/plans/M1b.md, question 3).
/// An error from a reference-cell layout, element, quadrature rule, tabulation or node map.
///
/// The mesh functions of the facade crate `tetraloma` report the matching `GridError` variants
/// instead. The enum is `#[non_exhaustive]`, so a `match` on it needs a `_` arm. The chapter
/// [Lagrange nodes] says which node families each cell supports.
///
/// # Example
///
/// ```
/// use tetraloma_dims::CellType;
/// use tetraloma_refcell::{NodeFamily, RefCellError};
///
/// let err = RefCellError::UnsupportedNodeFamily { cell: CellType::Pyramid, family: NodeFamily::Gll };
/// assert_eq!(err.to_string(), "node family Gll is not supported on Pyramid cells");
/// ```
///
/// [Lagrange nodes]: https://tetracl.github.io/tetraloma/elements/lagrange.html#node-families
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefCellError {
    /// The node family is not offered on this cell: GLL exists on intervals, triangles,
    /// quadrilaterals, tetrahedra, hexahedra and prisms, not on pyramids.
    UnsupportedNodeFamily {
        /// The cell type.
        cell: CellType,
        /// The requested node family.
        family: NodeFamily,
    },
    /// The degree is not supported: layouts, elements and node maps need p ≥ 1.
    UnsupportedDegree {
        /// The cell type.
        cell: CellType,
        /// The requested degree.
        degree: u32,
    },
    /// Tabulation supports derivative orders 0 and 1 only.
    UnsupportedDerivativeOrder(u32),
    /// A flat point array whose length is not a multiple of the cell's topological dimension.
    PointsShape {
        /// The length of the point array.
        len: usize,
        /// The topological dimension of the cell.
        tdim: usize,
    },
    /// The file format has no node ordering for this cell and degree.
    NotInFormat {
        /// The file format, `"Gmsh"` or `"VTK"`.
        format: &'static str,
        /// The cell type.
        cell: CellType,
        /// The degree.
        degree: u32,
    },
    /// The matrix passed to [`DenseLu::factor`](crate::DenseLu::factor) (such as a generalised
    /// Vandermonde matrix) is singular to working precision.
    Singular,
    // Design: section 27 ("Quadrature rules and point sets"); ADR-0081.
    /// The requested quadrature rule type is not offered on this cell.
    UnsupportedQuadratureType {
        /// The cell type.
        cell: CellType,
        /// The requested quadrature rule type.
        kind: QuadratureType,
    },
    // Design: section 27 ("Quadrature rules and point sets"); ADR-0081.
    /// The sub-entity dimension or index does not exist on this cell.
    InvalidSubEntity {
        /// The cell type.
        cell: CellType,
        /// The sub-entity topological dimension.
        dim: usize,
        /// The sub-entity index within dimension `dim`.
        index: usize,
    },
    // Design: section 27 ("Quadrature rules and point sets"); ADR-0081.
    /// The orientation bit field or entity type is not valid for sub-entity reorientation.
    InvalidOrientationBits {
        /// The sub-entity cell type.
        entity: CellType,
        /// The orientation bit field.
        bits: u32,
    },
    // Design: section 27 ("Quadrature rules and point sets"); ADR-0081.
    /// The quadrature rule is not invariant under the requested sub-entity reorientation.
    UnsymmetricRule {
        /// The sub-entity cell type.
        entity: CellType,
    },
    // Design: section 27 ("Quadrature rules and point sets"); ADR-0081.
    /// The vertex permutation is not a combinatorial symmetry of this cell.
    InvalidVertexPermutation {
        /// The cell type.
        cell: CellType,
    },
    // Design: section 27 ("Quadrature rules and point sets"); ADR-0081.
    /// Two cell types that must agree differ.
    CellMismatch {
        /// The expected cell type.
        expected: CellType,
        /// The cell type given.
        found: CellType,
    },
}

impl fmt::Display for RefCellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedNodeFamily { cell, family } => {
                write!(
                    f,
                    "node family {family:?} is not supported on {cell:?} cells"
                )
            }
            Self::UnsupportedDegree { cell, degree } => {
                write!(
                    f,
                    "degree {degree} is not supported on {cell:?} cells (p >= 1 required)"
                )
            }
            Self::UnsupportedDerivativeOrder(n) => {
                write!(f, "derivative order {n} is not supported (0 or 1)")
            }
            Self::PointsShape { len, tdim } => write!(
                f,
                "a point array of length {len} is not a whole number of points of dimension {tdim}"
            ),
            Self::NotInFormat {
                format,
                cell,
                degree,
            } => {
                write!(
                    f,
                    "{format} has no node ordering for {cell:?} cells of degree {degree}"
                )
            }
            Self::Singular => write!(f, "the matrix is singular to working precision"),
            Self::UnsupportedQuadratureType { cell, kind } => {
                write!(
                    f,
                    "quadrature type {kind:?} is not supported on {cell:?} cells"
                )
            }
            Self::InvalidSubEntity { cell, dim, index } => {
                write!(
                    f,
                    "sub-entity ({dim}, {index}) does not exist on {cell:?} cells"
                )
            }
            Self::InvalidOrientationBits { entity, bits } => {
                write!(f, "orientation bits {bits:#x} are not valid for {entity:?}")
            }
            Self::UnsymmetricRule { entity } => write!(
                f,
                "the quadrature rule is not invariant under the reorientation of {entity:?}"
            ),
            Self::InvalidVertexPermutation { cell } => {
                write!(f, "the vertex permutation is not a symmetry of {cell:?}")
            }
            Self::CellMismatch { expected, found } => {
                write!(f, "cell mismatch: expected {expected:?}, found {found:?}")
            }
        }
    }
}

impl std::error::Error for RefCellError {}

#[cfg(test)]
mod tests {
    use super::RefCellError;
    use crate::QuadratureType;
    use tetraloma_dims::CellType;

    #[test]
    fn m13_variants_display() {
        let cases = [
            (
                RefCellError::UnsupportedQuadratureType {
                    cell: CellType::Pyramid,
                    kind: QuadratureType::Symmetric,
                },
                "quadrature type Symmetric is not supported on Pyramid cells",
            ),
            (
                RefCellError::InvalidSubEntity {
                    cell: CellType::Triangle,
                    dim: 1,
                    index: 3,
                },
                "sub-entity (1, 3) does not exist on Triangle cells",
            ),
            (
                RefCellError::InvalidOrientationBits {
                    entity: CellType::Triangle,
                    bits: 3,
                },
                "orientation bits 0x3 are not valid for Triangle",
            ),
            (
                RefCellError::UnsymmetricRule {
                    entity: CellType::Triangle,
                },
                "the quadrature rule is not invariant under the reorientation of Triangle",
            ),
            (
                RefCellError::InvalidVertexPermutation {
                    cell: CellType::Quadrilateral,
                },
                "the vertex permutation is not a symmetry of Quadrilateral",
            ),
            (
                RefCellError::CellMismatch {
                    expected: CellType::Triangle,
                    found: CellType::Quadrilateral,
                },
                "cell mismatch: expected Triangle, found Quadrilateral",
            ),
        ];
        for (err, want) in cases {
            assert_eq!(err.to_string(), want);
        }
    }
}
