// Escape a SQL string for embedding as a Python string literal.
//
// Placeholder rewriting (e.g. $1 → %(name)s for psycopg) lives per-driver
// because only some drivers need it. This helper operates on already-
// rewritten SQL.
pub fn escape_sql(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
