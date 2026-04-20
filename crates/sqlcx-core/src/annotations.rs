use std::collections::HashMap;

use regex::Regex;

use crate::ir::{JsonShape, QueryCommand};

// ── Public types ──────────────────────────────────────────────────────────────

pub struct QueryHeader {
    pub name: String,
    pub command: QueryCommand,
}

pub struct Annotations {
    pub enums: HashMap<String, Vec<String>>,
    pub json_shapes: HashMap<String, JsonShape>,
    pub param_overrides: HashMap<u32, String>,
    pub query_header: Option<QueryHeader>,
}

impl Annotations {
    fn new() -> Self {
        Self {
            enums: HashMap::new(),
            json_shapes: HashMap::new(),
            param_overrides: HashMap::new(),
            query_header: None,
        }
    }
}

// ── Regex patterns ────────────────────────────────────────────────────────────

fn header_re() -> Regex {
    Regex::new(r"--\s*name:\s*(\w+)\s+:(one|many|exec(?:result)?)").unwrap()
}

fn param_re() -> Regex {
    Regex::new(r"--\s*@param\s+\$(\d+)\s+(\w+)").unwrap()
}

fn enum_re() -> Regex {
    Regex::new(r#"--\s*@enum\s*\(\s*(.*?)\s*\)"#).unwrap()
}

fn json_re() -> Regex {
    Regex::new(r"--\s*@json\s*\(\s*([\s\S]+?)\s*\)\s*$").unwrap()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns the first word of the first non-empty, non-comment line after `start`.
fn find_next_column_name<'a>(lines: &[&'a str], start: usize) -> Option<&'a str> {
    for line in lines.iter().skip(start) {
        let t = line.trim();
        if t.is_empty() || t.starts_with("--") {
            continue;
        }
        return t.split_whitespace().next();
    }
    None
}

/// Split a string by commas, ignoring commas inside nested braces/parens.
#[cfg(test)]
fn split_top_level(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;

    for (i, ch) in s.char_indices() {
        match ch {
            '{' | '(' => depth += 1,
            '}' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

fn parse_enum_values(inner: &str) -> Vec<String> {
    let re = Regex::new(r#""([^"]*?)""#).unwrap();
    re.captures_iter(inner).map(|c| c[1].to_string()).collect()
}

// ── JSON shape parser ─────────────────────────────────────────────────────────

struct JsonParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn parse(mut self) -> Result<JsonShape, String> {
        let shape = self.parse_type()?;
        self.skip_ws();
        if self.pos < self.input.len() {
            return Err(format!(
                "unexpected trailing content at pos {}: {:?}",
                self.pos,
                &self.input[self.pos..].chars().take(10).collect::<String>()
            ));
        }
        Ok(shape)
    }

    fn parse_type(&mut self) -> Result<JsonShape, String> {
        self.skip_ws();
        let mut shape = if self.peek() == Some('{') {
            self.parse_object()?
        } else {
            self.parse_primitive()?
        };

        // array suffix []
        self.skip_ws();
        while self.look_ahead("[]") {
            self.pos += 2;
            self.skip_ws();
            shape = JsonShape::Array {
                element: Box::new(shape),
            };
        }

        // nullable suffix ?
        if self.peek() == Some('?') {
            self.pos += 1;
            shape = JsonShape::Nullable {
                inner: Box::new(shape),
            };
        }

        Ok(shape)
    }

    fn parse_primitive(&mut self) -> Result<JsonShape, String> {
        self.skip_ws();
        if self.match_word("string") {
            return Ok(JsonShape::String);
        }
        if self.match_word("number") {
            return Ok(JsonShape::Number);
        }
        if self.match_word("boolean") {
            return Ok(JsonShape::Boolean);
        }
        Err(format!(
            "unexpected token at pos {}: {:?}",
            self.pos,
            self.input[self.pos..].chars().take(10).collect::<String>()
        ))
    }

    fn parse_object(&mut self) -> Result<JsonShape, String> {
        self.consume('{')?;
        self.skip_ws();
        let mut fields = HashMap::new();

        if self.peek() != Some('}') {
            self.parse_field(&mut fields)?;
            while self.peek() == Some(',') {
                self.pos += 1;
                self.skip_ws();
                if self.peek() == Some('}') {
                    break; // trailing comma
                }
                self.parse_field(&mut fields)?;
            }
        }

        self.consume('}')?;
        Ok(JsonShape::Object { fields })
    }

    fn parse_field(&mut self, fields: &mut HashMap<String, JsonShape>) -> Result<(), String> {
        self.skip_ws();
        let name = self.read_identifier()?;
        self.skip_ws();
        self.consume(':')?;
        self.skip_ws();
        let shape = self.parse_type()?;
        self.skip_ws();
        fields.insert(name, shape);
        Ok(())
    }

    fn read_identifier(&mut self) -> Result<String, String> {
        self.skip_ws();
        let start = self.pos;
        while self.pos < self.input.len()
            && self
                .input
                .as_bytes()
                .get(self.pos)
                .map(|b| b.is_ascii_alphanumeric() || *b == b'_')
                .unwrap_or(false)
        {
            self.pos += 1;
        }
        if self.pos == start {
            return Err(format!("expected identifier at pos {}", self.pos));
        }
        Ok(self.input[start..self.pos].to_string())
    }

    fn skip_ws(&mut self) {
        while self.pos < self.input.len() && self.input.as_bytes()[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.skip_ws();
        self.input[self.pos..].chars().next()
    }

    fn look_ahead(&self, s: &str) -> bool {
        self.input[self.pos..].starts_with(s)
    }

    fn match_word(&mut self, word: &str) -> bool {
        if self.input[self.pos..].starts_with(word) {
            let after = self.pos + word.len();
            let next_is_word_char = self
                .input
                .as_bytes()
                .get(after)
                .map(|b| b.is_ascii_alphanumeric() || *b == b'_')
                .unwrap_or(false);
            if !next_is_word_char {
                self.pos = after;
                return true;
            }
        }
        false
    }

    fn consume(&mut self, ch: char) -> Result<(), String> {
        self.skip_ws();
        match self.input[self.pos..].chars().next() {
            Some(c) if c == ch => {
                self.pos += ch.len_utf8();
                Ok(())
            }
            other => Err(format!(
                "expected {:?} at pos {}, got {:?}",
                ch, self.pos, other
            )),
        }
    }
}

fn parse_json_shape(body: &str) -> Option<JsonShape> {
    match JsonParser::new(body.trim()).parse() {
        Ok(shape) => Some(shape),
        Err(e) => {
            eprintln!("warning: failed to parse @json annotation: {e}");
            None
        }
    }
}

// ── Main extraction function ──────────────────────────────────────────────────

/// Extract annotations from SQL. Returns `(cleaned_sql, annotations)`.
/// Annotation lines are removed; regular comments are preserved.
pub fn extract_annotations(sql: &str) -> (String, Annotations) {
    let lines: Vec<&str> = sql.lines().collect();
    let mut annotations = Annotations::new();
    let mut kept_lines: Vec<&str> = Vec::new();

    let h_re = header_re();
    let p_re = param_re();
    let e_re = enum_re();
    let j_re = json_re();

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Query header: -- name: Foo :one
        if let Some(cap) = h_re.captures(trimmed) {
            let name = cap[1].to_string();
            let command = match &cap[2] {
                "one" => QueryCommand::One,
                "many" => QueryCommand::Many,
                "execresult" => QueryCommand::ExecResult,
                _ => QueryCommand::Exec,
            };
            annotations.query_header = Some(QueryHeader { name, command });
            i += 1;
            continue;
        }

        // Param override: -- @param $1 name
        if let Some(cap) = p_re.captures(trimmed) {
            let idx: u32 = cap[1].parse().unwrap_or(0);
            let name = cap[2].to_string();
            annotations.param_overrides.insert(idx, name);
            i += 1;
            continue;
        }

        // Enum annotation: -- @enum("a", "b")
        if let Some(cap) = e_re.captures(trimmed) {
            let values = parse_enum_values(&cap[1]);
            if !values.is_empty()
                && let Some(col) = find_next_column_name(&lines, i + 1)
            {
                annotations.enums.insert(col.to_lowercase(), values);
            }
            i += 1;
            continue;
        }

        // JSON annotation: -- @json({ ... })
        if let Some(cap) = j_re.captures(trimmed) {
            if let Some(shape) = parse_json_shape(&cap[1])
                && let Some(col) = find_next_column_name(&lines, i + 1)
            {
                annotations.json_shapes.insert(col.to_lowercase(), shape);
            }
            i += 1;
            continue;
        }

        kept_lines.push(line);
        i += 1;
    }

    let cleaned = kept_lines.join("\n");
    (cleaned, annotations)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_query_header() {
        let sql = "-- name: GetUser :one\nSELECT * FROM users WHERE id = $1;";
        let (cleaned, ann) = extract_annotations(sql);
        let header = ann.query_header.unwrap();
        assert_eq!(header.name, "GetUser");
        assert_eq!(header.command, QueryCommand::One);
        assert!(!cleaned.contains("-- name:"));
    }

    #[test]
    fn extract_enum_annotation() {
        let sql = "-- @enum(\"draft\", \"published\", \"archived\")\nstatus TEXT NOT NULL";
        let (_, ann) = extract_annotations(sql);
        let values = ann.enums.get("status").unwrap();
        assert_eq!(values, &vec!["draft", "published", "archived"]);
    }

    #[test]
    fn extract_json_annotation() {
        let sql = "-- @json({ theme: string, notifications: boolean })\npreferences JSONB";
        let (_, ann) = extract_annotations(sql);
        let shape = ann.json_shapes.get("preferences").unwrap();
        match shape {
            JsonShape::Object { fields } => {
                assert!(fields.contains_key("theme"));
                assert!(fields.contains_key("notifications"));
            }
            _ => panic!("expected Object shape"),
        }
    }

    #[test]
    fn extract_param_override() {
        let sql = "-- @param $1 start_date\n-- @param $2 end_date\nSELECT * FROM users;";
        let (_, ann) = extract_annotations(sql);
        assert_eq!(ann.param_overrides.get(&1), Some(&"start_date".to_string()));
        assert_eq!(ann.param_overrides.get(&2), Some(&"end_date".to_string()));
    }

    #[test]
    fn strips_annotation_lines_from_sql() {
        let sql = "-- name: GetUser :one\n-- @param $1 user_id\nSELECT * FROM users WHERE id = $1;";
        let (cleaned, _) = extract_annotations(sql);
        assert!(!cleaned.contains("@param"));
        assert!(!cleaned.contains("-- name:"));
        assert!(cleaned.contains("SELECT"));
    }

    #[test]
    fn regular_comments_are_preserved() {
        let sql = "-- This is a regular comment\nSELECT 1;";
        let (cleaned, _) = extract_annotations(sql);
        assert!(cleaned.contains("-- This is a regular comment"));
        assert!(cleaned.contains("SELECT 1;"));
    }

    #[test]
    fn query_command_many() {
        let sql = "-- name: ListUsers :many\nSELECT * FROM users;";
        let (_, ann) = extract_annotations(sql);
        assert_eq!(ann.query_header.unwrap().command, QueryCommand::Many);
    }

    #[test]
    fn query_command_exec() {
        let sql = "-- name: DeleteUser :exec\nDELETE FROM users WHERE id = $1;";
        let (_, ann) = extract_annotations(sql);
        assert_eq!(ann.query_header.unwrap().command, QueryCommand::Exec);
    }

    #[test]
    fn query_command_execresult() {
        let sql = "-- name: UpdateUser :execresult\nUPDATE users SET name = $1 WHERE id = $2;";
        let (_, ann) = extract_annotations(sql);
        assert_eq!(ann.query_header.unwrap().command, QueryCommand::ExecResult);
    }

    #[test]
    fn json_array_shape() {
        let sql = "-- @json(string[])\ntags TEXT[]";
        let (_, ann) = extract_annotations(sql);
        let shape = ann.json_shapes.get("tags").unwrap();
        match shape {
            JsonShape::Array { element } => {
                assert!(matches!(**element, JsonShape::String));
            }
            _ => panic!("expected Array shape"),
        }
    }

    #[test]
    fn json_nullable_shape() {
        let sql = "-- @json(string?)\nnickname TEXT";
        let (_, ann) = extract_annotations(sql);
        let shape = ann.json_shapes.get("nickname").unwrap();
        match shape {
            JsonShape::Nullable { inner } => {
                assert!(matches!(**inner, JsonShape::String));
            }
            _ => panic!("expected Nullable shape"),
        }
    }

    #[test]
    fn split_top_level_nested() {
        let parts = split_top_level("a, { b: c, d: e }, f");
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].trim(), "a");
        assert_eq!(parts[1].trim(), "{ b: c, d: e }");
        assert_eq!(parts[2].trim(), "f");
    }

    #[test]
    fn empty_sql_no_panic() {
        let (cleaned, ann) = extract_annotations("");
        assert_eq!(cleaned, "");
        assert!(ann.query_header.is_none());
        assert!(ann.enums.is_empty());
    }
}
