// Design: section 26 ("Finite elements"), Listing E1; ADR-0071.
//! Errors of tetraloma-element.

use std::fmt;

use tetraloma_dims::CellType;
use tetraloma_refcell::RefCellError;

use crate::{Family, Variant};

// Design: section 26 ("Finite elements"), Listing E1.
/// An error returned when looking up or tabulating a finite element.
///
/// See [Families and degrees] for the supported combinations of family, cell, degree and variant.
///
/// # Example
///
/// ```
/// use tetraloma_dims::CellType;
/// use tetraloma_element::{Element, ElementError, ElementKey, Family, Variant};
/// use tetraloma_refcell::NodeFamily;
///
/// let key = ElementKey {
///     family: Family::Lagrange, cell: CellType::Pyramid, degree: 2, variant: Variant::Nodal(NodeFamily::Gll),
/// };
/// let err = Element::get(key).unwrap_err();
/// assert!(matches!(err, ElementError::Unsupported { cell: CellType::Pyramid, degree: 2, .. }));
/// assert!(err.to_string().contains("Pyramid"));
/// ```
///
/// [Families and degrees]: https://tetracl.github.io/tetraloma/fem/elements.html#families-and-degrees
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ElementError {
    /// The combination of family, cell type, degree and variant is not offered.
    Unsupported {
        /// The requested finite-element family.
        family: Family,
        /// The requested reference cell type.
        cell: CellType,
        /// The requested polyset superdegree.
        degree: u32,
        /// The requested element variant.
        variant: Variant,
    },
    /// An error from `tetraloma-refcell` (for example an unsupported derivative order or an
    /// invalid point-array length).
    RefCell(RefCellError),
}

impl fmt::Display for ElementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported {
                family,
                cell,
                degree,
                variant,
            } => write!(
                f,
                "unsupported element: {family:?} of degree {degree} ({variant:?}) on {cell:?} cells"
            ),
            Self::RefCell(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for ElementError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unsupported { .. } => None,
            Self::RefCell(err) => Some(err),
        }
    }
}

impl From<RefCellError> for ElementError {
    fn from(err: RefCellError) -> Self {
        Self::RefCell(err)
    }
}
