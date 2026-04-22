//! Shared helpers for TypeScript driver generators. Each driver implements
//! `TsDriverShape` (imports, connection type, placeholder rewrite, per-command
//! body). The shared skeleton emits row/params interfaces, SQL const, and the
//! typed `(conn, params)` signature. No client.ts, no class wrappers.

pub mod query_fn;
pub mod sql_escape;
pub mod types;

pub use query_fn::{
    BodyCtx, TsDriverShape, generate_driver_files, generate_queries_file, generate_query_function,
};
pub use sql_escape::json_stringify;
pub use types::{TsTypeMap, generate_params_type, generate_row_type, ts_type};
