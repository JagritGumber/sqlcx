// Normalize placeholders to the `?` positional form used by mysql2 and
// better-sqlite3. Accepts Postgres-style `$N` (emitted by the PG parser)
// and native `?` (emitted by the MySQL and SQLite parsers). Returns the
// rewritten SQL and the parameter indices in SQL occurrence order — the
// shared query-function skeleton uses those indices to build the typed
// values array.

pub fn rewrite_to_qmark(sql: &str) -> (String, Vec<u32>) {
    let mut result = String::with_capacity(sql.len());
    let mut indices = Vec::new();
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            let mut num_str = String::new();
            while chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                num_str.push(chars.next().unwrap());
            }
            result.push('?');
            indices.push(num_str.parse::<u32>().unwrap_or(0));
        } else if c == '?' {
            result.push('?');
            indices.push(indices.len() as u32 + 1);
        } else {
            result.push(c);
        }
    }
    (result, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dollar_n_rewrite_preserves_occurrence_indices() {
        let (sql, idx) = rewrite_to_qmark("WHERE a = $2 AND b = $1");
        assert_eq!(sql, "WHERE a = ? AND b = ?");
        assert_eq!(idx, vec![2, 1]);
    }

    #[test]
    fn reused_dollar_n_tracks_each_occurrence() {
        let (sql, idx) = rewrite_to_qmark("WHERE a = $1 OR b = $1");
        assert_eq!(sql, "WHERE a = ? OR b = ?");
        assert_eq!(idx, vec![1, 1]);
    }

    #[test]
    fn native_qmark_input_tracks_1_based_occurrence() {
        let (sql, idx) = rewrite_to_qmark("WHERE a = ? AND b = ?");
        assert_eq!(sql, "WHERE a = ? AND b = ?");
        assert_eq!(idx, vec![1, 2]);
    }
}
