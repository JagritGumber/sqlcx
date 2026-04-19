//! Shared helpers for Python driver generators.
//!
//! Both psycopg and asyncpg map SQL types to the same Python types and emit
//! identical `@dataclass Row`/`Params` classes. What differs is the
//! placeholder syntax (psycopg uses `%(name)s` named params; asyncpg uses
//! the raw `$1` positional) and the async vs sync query-function body.
//! Those stay per-driver; this module owns the shared surface.

pub mod sql_escape;
pub mod types;

pub use sql_escape::escape_sql;
pub use types::{generate_params_class, generate_row_class, py_type};
