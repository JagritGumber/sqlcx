// ── Shared string utilities ────────────────────────────────────────────────────

/// Split PascalCase/camelCase words by inserting underscores before capital letters.
pub fn split_words(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() && i > 0 && chars[i - 1].is_lowercase() {
            out.push('_');
        }
        out.push(c);
    }
    out
}

/// Convert snake_case, PascalCase, or camelCase to PascalCase.
pub fn pascal_case(s: &str) -> String {
    split_words(s)
        .split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}

/// Convert snake_case, PascalCase, or camelCase to camelCase.
pub fn camel_case(s: &str) -> String {
    let p = pascal_case(s);
    let mut c = p.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
    }
}

/// Escape a string for embedding in a JS/TS double-quoted literal.
/// Mirrors: JSON.stringify(str).slice(1, -1)
pub fn escape_string(s: &str) -> String {
    let serialized = serde_json::to_string(s).unwrap_or_else(|_| format!("\"{}\"", s));
    // Strip the outer quotes that serde_json adds
    serialized[1..serialized.len() - 1].to_string()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_case_snake() {
        assert_eq!(pascal_case("user_status"), "UserStatus");
    }

    #[test]
    fn pascal_case_single() {
        assert_eq!(pascal_case("users"), "Users");
    }

    #[test]
    fn camel_case_snake() {
        assert_eq!(camel_case("get_user"), "getUser");
    }

    #[test]
    fn split_words_pascal() {
        assert_eq!(split_words("GetUser"), "Get_User");
    }

    #[test]
    fn escape_string_quotes() {
        assert_eq!(escape_string("he said \"hi\""), "he said \\\"hi\\\"");
    }
}
