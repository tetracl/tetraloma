// Design: section 26 ("Finite elements"), Listing E1; ADR-0071, ADR-0072.
// Stubs for M11.T0; each item loses `#[doc(hidden)]` and gains user documentation in the task
// that implements it (docs/plans/M11.md, Tasks).
//! Finite elements in Ciarlet's sense on the reference cells of tetraloma.

use std::sync::Arc;

use tetraloma_dims::CellType;
use tetraloma_refcell::{NodeFamily, RefCellError};

mod tensor;

pub use tensor::{Factor1d, TensorFactors};

#[doc(hidden)]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    Lagrange,
    N1curl,
    RaviartThomas,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Variant {
    Nodal(NodeFamily),
    Legendre,
}

#[doc(hidden)]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapType {
    Identity,
    CovariantPiola,
    ContravariantPiola,
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

#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementKey {
    pub family: Family,
    pub cell: CellType,
    pub degree: u32,
    pub variant: Variant,
}

#[doc(hidden)]
pub struct Element {
    _private: (),
}

impl Element {
    #[doc(hidden)]
    pub fn get(key: ElementKey) -> Result<Arc<Element>, ElementError> {
        let _ = key;
        unimplemented!("pending M11.T4")
    }

    #[doc(hidden)]
    pub fn key(&self) -> ElementKey {
        unimplemented!("pending M11.T4")
    }

    #[doc(hidden)]
    pub fn map_type(&self) -> MapType {
        unimplemented!("pending M11.T4")
    }

    #[doc(hidden)]
    pub fn reference_value_size(&self) -> usize {
        unimplemented!("pending M11.T4")
    }

    #[doc(hidden)]
    pub fn dim(&self) -> usize {
        unimplemented!("pending M11.T4")
    }

    #[doc(hidden)]
    pub fn entity_dofs(&self, d: usize, e: usize) -> &[usize] {
        let _ = (d, e);
        unimplemented!("pending M11.T4")
    }

    #[doc(hidden)]
    pub fn entity_closure_dofs(&self, d: usize, e: usize) -> &[usize] {
        let _ = (d, e);
        unimplemented!("pending M11.T4")
    }

    #[doc(hidden)]
    pub fn discontinuous_only(&self) -> bool {
        unimplemented!("pending M11.T4")
    }

    #[doc(hidden)]
    pub fn embedded_superdegree(&self) -> u32 {
        unimplemented!("pending M11.T4")
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

#[doc(hidden)]
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ElementError {
    Unsupported {
        family: Family,
        cell: CellType,
        degree: u32,
        variant: Variant,
    },
    RefCell(RefCellError),
}
