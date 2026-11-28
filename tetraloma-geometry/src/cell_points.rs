//! `CellPoints` (Listing Q1; ADR-0081).
// Stub for M13.T0b; implemented and documented in M13.T5.

use core::marker::PhantomData;

use cubecl::prelude::Client;
use tetraloma_device::{DeviceBuffer, Real};
use tetraloma_dims::TopoDim;
use tetraloma_topology::Result;

// Design: section 27 ("Quadrature rules and point sets"), Listing Q1; ADR-0081.
// Stub for M13.T0b; `#[doc(hidden)]` is removed and user documentation is added in M13.T5.
#[doc(hidden)]
pub struct CellPoints<F: Real, TD: TopoDim> {
    _marker: PhantomData<(F, TD)>,
}

impl<F: Real, TD: TopoDim> CellPoints<F, TD> {
    #[doc(hidden)]
    pub fn new(
        client: &Client,
        block: usize,
        cells: DeviceBuffer<u32>,
        points: DeviceBuffer<F>,
    ) -> Result<Self> {
        let _ = (client, block, cells, points);
        unimplemented!("pending M13.T5")
    }

    #[doc(hidden)]
    pub fn block(&self) -> usize {
        unimplemented!("pending M13.T5")
    }

    #[doc(hidden)]
    pub fn len(&self) -> usize {
        unimplemented!("pending M13.T5")
    }

    #[doc(hidden)]
    pub fn is_empty(&self) -> bool {
        unimplemented!("pending M13.T5")
    }

    #[doc(hidden)]
    pub fn cells(&self) -> &DeviceBuffer<u32> {
        unimplemented!("pending M13.T5")
    }

    #[doc(hidden)]
    pub fn points(&self) -> &DeviceBuffer<F> {
        unimplemented!("pending M13.T5")
    }

    #[doc(hidden)]
    pub fn client(&self) -> &Client {
        unimplemented!("pending M13.T5")
    }
}
