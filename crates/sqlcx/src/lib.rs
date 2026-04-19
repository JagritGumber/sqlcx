//! sqlcx CLI library
//!
//! Exposes the codegen pipeline so callers other than the CLI binary
//! (e.g. integration tests, future watch mode, embedded uses) can run
//! generation without shelling out.

pub mod pipeline;

pub use pipeline::run_pipeline;
