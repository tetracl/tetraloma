// Design: "Workspace layout", "Testing and validation"; ADR-0075.
//! Integration tests of tetraloma's foundation crates (`tetraloma-device` and `tetraloma-parallel`)
//! without a mesh crate in the normal dependency tree.
//!
//! This crate lives in the repository and is not published. It defines no Cargo features: runs on
//! a GPU backend enable the runtime feature on `tetraloma-testing-base` (for example
//! `--features tetraloma-testing-base/metal`). For user-facing documentation see the [user book].
//!
//! [user book]: https://tetracl.github.io/tetraloma/
