//! Shared helpers for TypeScript driver generators.
//!
//! Every TS driver emits the same row/params interfaces and the same SQL
//! escape logic; only the per-driver *body* of the query function and the
//! client.ts content differ. This module owns the shared surface so new
//! drivers only write a `TsTypeMap` impl plus their driver-specific body.

pub mod sql_escape;
pub mod types;

pub use sql_escape::json_stringify;
pub use types::{generate_params_type, generate_row_type, ts_type, TsTypeMap};
