//! Shared helpers for Rust driver generators.
//!
//! Both sqlx and tokio-postgres target Postgres with the same SQL→Rust
//! type mapping. What differs is the row-struct derives (sqlx adds
//! `sqlx::FromRow`, tokio-postgres doesn't) and the query-function body
//! (sqlx uses `query_as`, tokio-postgres uses `client.query_opt` etc).
//! Those stay per-driver; this module owns the shared type mapping.

pub mod types;

pub use types::{date_type, number_type, param_type, row_field_type, rust_type};
