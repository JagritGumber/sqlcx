//! Shared helpers for TypeScript driver generators.
//!
//! Every TS driver emits the same row/params interfaces and the same SQL
//! escape logic; only the per-driver *body* of the query function and the
//! client.ts content differ. For async-DatabaseClient-style drivers (pg,
//! bun-sql), `query_fn::generate_driver_files` renders the whole output
//! bundle from just a client.ts string plus a `TsTypeMap` impl. Drivers
//! that diverge (mysql2 placeholder rewrite, better-sqlite3 sync API)
//! keep their own query-function body.

pub mod query_fn;
pub mod sql_escape;
pub mod types;

pub use query_fn::{generate_driver_files, generate_query_function, generate_query_functions_file};
pub use sql_escape::json_stringify;
pub use types::{generate_params_type, generate_row_type, ts_type, TsTypeMap};
