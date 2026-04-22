//! Shared helpers for Python driver generators. Each driver implements
//! `PyDriverShape` (imports, connection type, placeholder rewrite, params
//! arg formatting, per-command body). The shared skeleton emits dataclass
//! row/params classes, SQL const, and the `def`/`async def` signature.
//! No client.py, no wrappers — queries.py imports the driver package directly.

pub mod query_fn;
pub mod sql_escape;
pub mod types;

pub use query_fn::{
    PyBodyCtx, PyDriverShape, generate_driver_files, generate_queries_file, generate_query_function,
};
pub use sql_escape::escape_sql;
pub use types::{DefaultPyTypeMap, PyTypeMap, generate_params_class, generate_row_class, py_type};
