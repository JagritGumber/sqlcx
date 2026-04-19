//! Shared helpers for Go driver generators.
//!
//! database/sql and pgx emit identical row structs, result structs, scan-fields,
//! function signatures, and SQL constant helpers. Only the client (DBTX
//! interface) and the query-function body differ (database/sql uses
//! ExecContext/QueryRowContext; pgx uses pgconn.CommandTag and pgx.Rows).
//! Those stay per-driver; this module owns the shared surface.

pub mod codegen;
pub mod naming;

pub use codegen::{
    escape_sql, func_params, generate_result_struct, generate_row_struct, query_args, scan_fields,
};
pub use naming::{lcfirst, sql_const_name};
