//! Shared helpers for Python driver generators.
//!
//! psycopg and asyncpg map Postgres types to the same idiomatic Python
//! types (bool, datetime, Any for JSON, bytes, etc). sqlite3 diverges:
//! SQLite doesn't have native Boolean/Date/Json, so it maps them to
//! int/str/str. The `PyTypeMap` trait captures that divergence — each
//! driver provides a small type map struct; `py_type` and the
//! row/params class generators take any `PyTypeMap` via generics.

pub mod sql_escape;
pub mod types;

pub use sql_escape::escape_sql;
pub use types::{DefaultPyTypeMap, PyTypeMap, generate_params_class, generate_row_class, py_type};
