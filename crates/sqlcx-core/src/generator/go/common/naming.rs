// Naming helpers shared across Go driver generators.

use crate::utils::pascal_case;

/// Generate the SQL constant name for a query: `getUserSQL`, `listUsersSQL`, etc.
pub fn sql_const_name(query_name: &str) -> String {
    format!("{}SQL", lcfirst(&pascal_case(query_name)))
}

/// Lowercase the first character of a string.
pub fn lcfirst(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
    }
}
