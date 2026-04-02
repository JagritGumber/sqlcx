# sqlcx Rust Rewrite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite sqlcx from TypeScript to Rust as a Cargo workspace with compiled-in plugins, producing an identical TypeScript code generation pipeline (PostgreSQL → IR → TypeBox + Bun.sql).

**Architecture:** Two-crate Cargo workspace — `sqlcx-core` (library: IR, parser, generators, cache, config) and `sqlcx` (binary: CLI via clap). Bottom-up build: IR → annotations → parser → generators → cache → config → CLI → e2e.

**Tech Stack:** Rust, serde/serde_json, sqlparser-rs, clap, sha2, thiserror, toml, glob, insta, tempfile, schemars

**Spec:** `docs/superpowers/specs/2026-03-21-sqlcx-rust-rewrite-design.md`
**TS Reference:** `src/` directory contains the TypeScript prototype — match its output exactly.

---

## Branch Strategy

Each task runs in its own worktree branch off `main`. Tasks are **sequential** — each branch is created from the merged state of the previous task.

```
main ←── feat/rust-01-workspace-scaffolding
     ←── feat/rust-02-ir-types
     ←── feat/rust-03-error-types
     ←── feat/rust-04-annotations
     ←── feat/rust-05-param-naming
     ←── feat/rust-06-postgres-parser
     ←── feat/rust-07-generator-traits
     ←── feat/rust-08-typebox-generator       (can parallel with 09)
     ←── feat/rust-09-bun-sql-generator       (can parallel with 08)
     ←── feat/rust-10-typescript-plugin
     ←── feat/rust-11-config
     ←── feat/rust-12-cache
     ←── feat/rust-13-cli
     ←── feat/rust-14-e2e-tests
     ←── feat/rust-15-npm-packaging
```

**Parallelizable tasks:**
- Tasks 8 and 9 can run in parallel (both depend on Task 7, neither depends on each other).
- Tasks 11 and 12 can run in parallel (both depend on Task 7 for config structs, neither depends on each other or on Task 10).

**Deferred (not in this plan):** CI/CD pipeline (GitHub Actions for cross-compilation and publishing), PyPI/Homebrew packaging scripts. These will be separate follow-up tasks after the core binary is working.

**Cargo.lock:** Should be committed since the workspace includes a binary crate (Rust convention).

---

## File Map

```
sqlcx-rust/
├── Cargo.toml                           # Workspace root
├── crates/
│   ├── sqlcx-core/
│   │   ├── Cargo.toml                   # lib crate dependencies
│   │   └── src/
│   │       ├── lib.rs                   # pub mod declarations + re-exports
│   │       ├── ir.rs                    # IR structs (Task 2)
│   │       ├── error.rs                 # SqlcxError enum (Task 3)
│   │       ├── annotations.rs           # @enum/@json/@param extractor (Task 4)
│   │       ├── param_naming.rs          # Parameter name inference (Task 5)
│   │       ├── parser/
│   │       │   ├── mod.rs               # DatabaseParser trait + resolve_parser (Task 6)
│   │       │   └── postgres.rs          # PostgreSQL parser via sqlparser-rs (Task 6)
│   │       ├── generator/
│   │       │   ├── mod.rs               # LanguagePlugin, SchemaGenerator, DriverGenerator traits (Task 7)
│   │       │   └── typescript/
│   │       │       ├── mod.rs           # TypeScript plugin orchestrator (Task 10)
│   │       │       ├── typebox.rs       # TypeBox schema generator (Task 8)
│   │       │       └── bun_sql.rs       # Bun.sql driver generator (Task 9)
│   │       ├── config.rs               # TOML/JSON config structs (Task 7) + loading (Task 11)
│   │       └── cache.rs                # SHA-256 IR caching (Task 12)
│   └── sqlcx/
│       ├── Cargo.toml                   # bin crate, depends on sqlcx-core
│       └── src/
│           └── main.rs                  # CLI: generate, check, init, schema (Task 13)
├── tests/
│   ├── fixtures/                        # Copied from TS version (Task 1)
│   │   ├── schema.sql
│   │   └── queries/
│   │       └── users.sql
│   ├── cli.rs                           # CLI integration tests (Task 13)
│   └── e2e.rs                           # End-to-end pipeline test (Task 14)
├── npm/                                 # npm packaging (Task 15)
│   ├── sqlcx/
│   │   ├── package.json
│   │   └── bin/
│   │       └── sqlcx                    # JS shim
│   ├── darwin-arm64/package.json
│   ├── darwin-x64/package.json
│   ├── linux-x64-gnu/package.json
│   ├── linux-arm64-gnu/package.json
│   └── win32-x64-msvc/package.json
└── schema/
    └── sqlcx-config.schema.json         # Generated (Task 11)
```

---

### Task 1: Workspace Scaffolding

**Branch:** `feat/rust-01-workspace-scaffolding` (from `main`)
**Depends on:** nothing
**Files:**
- Create: `sqlcx-rust/Cargo.toml`
- Create: `sqlcx-rust/crates/sqlcx-core/Cargo.toml`
- Create: `sqlcx-rust/crates/sqlcx-core/src/lib.rs`
- Create: `sqlcx-rust/crates/sqlcx/Cargo.toml`
- Create: `sqlcx-rust/crates/sqlcx/src/main.rs`
- Create: `sqlcx-rust/tests/fixtures/schema.sql` (copy from `tests/fixtures/schema.sql`)
- Create: `sqlcx-rust/tests/fixtures/queries/users.sql` (copy from `tests/fixtures/queries/users.sql`)
- Create: `sqlcx-rust/.gitignore`

- [ ] **Step 1: Create workspace root Cargo.toml**

Write `sqlcx-rust/Cargo.toml`:
```toml
[workspace]
members = ["crates/sqlcx-core", "crates/sqlcx"]
resolver = "2"

[workspace.package]
version = "1.0.0"
edition = "2021"
license = "MIT"
repository = "https://github.com/JagritGumber/sqlcx"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
```

- [ ] **Step 2: Create sqlcx-core crate**

Write `sqlcx-rust/crates/sqlcx-core/Cargo.toml`:
```toml
[package]
name = "sqlcx-core"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "SQL-first cross-language type-safe code generator — core library"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
sqlparser = { version = "0.53", features = ["visitor"] }
sha2 = "0.10"
toml = "0.8"
glob = "0.3"
tempfile = "3"
regex = "1"
schemars = "0.8"

[dev-dependencies]
insta = { version = "1", features = ["json"] }
```

Write `sqlcx-rust/crates/sqlcx-core/src/lib.rs`:
```rust
pub mod ir;
pub mod error;
```

- [ ] **Step 3: Create sqlcx binary crate**

Write `sqlcx-rust/crates/sqlcx/Cargo.toml`:
```toml
[package]
name = "sqlcx"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "SQL-first cross-language type-safe code generator — CLI"

[[bin]]
name = "sqlcx"
path = "src/main.rs"

[dependencies]
sqlcx-core = { path = "../sqlcx-core" }
clap = { version = "4", features = ["derive"] }
```

Write `sqlcx-rust/crates/sqlcx/src/main.rs`:
```rust
fn main() {
    println!("sqlcx — SQL-first code generator");
}
```

- [ ] **Step 4: Copy test fixtures from TS version**

```bash
mkdir -p sqlcx-rust/tests/fixtures/queries
cp tests/fixtures/schema.sql sqlcx-rust/tests/fixtures/schema.sql
cp tests/fixtures/queries/users.sql sqlcx-rust/tests/fixtures/queries/users.sql
```

- [ ] **Step 5: Create .gitignore**

Write `sqlcx-rust/.gitignore`:
```
/target
.sqlcx/
```

- [ ] **Step 6: Verify workspace builds**

```bash
cd sqlcx-rust && cargo build
```
Expected: successful build, no errors.

- [ ] **Step 7: Commit (include Cargo.lock)**

```bash
git add sqlcx-rust/
git commit -m "feat(rust): scaffold Cargo workspace with sqlcx-core and sqlcx crates"
```

---

### Task 2: IR Type Definitions

**Branch:** `feat/rust-02-ir-types` (from `main` after Task 1 merged)
**Depends on:** Task 1
**Files:**
- Create: `sqlcx-rust/crates/sqlcx-core/src/ir.rs`

- [ ] **Step 1: Write the IR round-trip test**

Add tests at the bottom of `sqlcx-rust/crates/sqlcx-core/src/ir.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ir_round_trip_json() {
        let ir = SqlcxIR {
            tables: vec![TableDef {
                name: "users".to_string(),
                columns: vec![ColumnDef {
                    name: "id".to_string(),
                    alias: None,
                    source_table: None,
                    sql_type: SqlType {
                        raw: "SERIAL".to_string(),
                        normalized: "serial".to_string(),
                        category: SqlTypeCategory::Number,
                        element_type: None,
                        enum_name: None,
                        enum_values: None,
                        json_shape: None,
                    },
                    nullable: false,
                    has_default: true,
                }],
                primary_key: vec!["id".to_string()],
                unique_constraints: vec![],
            }],
            queries: vec![],
            enums: vec![],
        };

        let json = serde_json::to_string_pretty(&ir).unwrap();
        let deserialized: SqlcxIR = serde_json::from_str(&json).unwrap();
        assert_eq!(ir.tables.len(), deserialized.tables.len());
        assert_eq!(ir.tables[0].name, deserialized.tables[0].name);
        assert_eq!(ir.tables[0].columns[0].name, deserialized.tables[0].columns[0].name);
    }

    #[test]
    fn sql_type_category_serializes_lowercase() {
        let cat = SqlTypeCategory::String;
        let json = serde_json::to_value(&cat).unwrap();
        assert_eq!(json, serde_json::json!("string"));

        let cat = SqlTypeCategory::Binary;
        let json = serde_json::to_value(&cat).unwrap();
        assert_eq!(json, serde_json::json!("binary"));
    }

    #[test]
    fn json_shape_serializes_with_kind_tag() {
        let shape = JsonShape::Object {
            fields: {
                let mut m = std::collections::HashMap::new();
                m.insert("theme".to_string(), JsonShape::String);
                m
            },
        };
        let json = serde_json::to_value(&shape).unwrap();
        assert_eq!(json["kind"], "object");
        assert_eq!(json["fields"]["theme"]["kind"], "string");
    }

    #[test]
    fn query_command_serializes_lowercase() {
        let cmd = QueryCommand::ExecResult;
        let json = serde_json::to_value(&cmd).unwrap();
        assert_eq!(json, serde_json::json!("execresult"));
    }

    #[test]
    fn camel_case_json_keys() {
        let ir = SqlcxIR {
            tables: vec![TableDef {
                name: "t".to_string(),
                columns: vec![ColumnDef {
                    name: "c".to_string(),
                    alias: None,
                    source_table: None,
                    sql_type: SqlType {
                        raw: "INT".to_string(),
                        normalized: "int".to_string(),
                        category: SqlTypeCategory::Number,
                        element_type: None,
                        enum_name: None,
                        enum_values: None,
                        json_shape: None,
                    },
                    nullable: false,
                    has_default: true,
                }],
                primary_key: vec![],
                unique_constraints: vec![],
            }],
            queries: vec![],
            enums: vec![],
        };
        let json = serde_json::to_string(&ir).unwrap();
        assert!(json.contains("\"primaryKey\""));
        assert!(json.contains("\"hasDefault\""));
        assert!(!json.contains("\"primary_key\""));
        assert!(!json.contains("\"has_default\""));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core
```
Expected: FAIL — structs not defined yet.

- [ ] **Step 3: Write IR structs**

Write the struct definitions in `sqlcx-rust/crates/sqlcx-core/src/ir.rs` above the test module:
```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SqlcxIR {
    pub tables: Vec<TableDef>,
    pub queries: Vec<QueryDef>,
    pub enums: Vec<EnumDef>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TableDef {
    pub name: String,
    pub columns: Vec<ColumnDef>,
    pub primary_key: Vec<String>,
    pub unique_constraints: Vec<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ColumnDef {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_table: Option<String>,
    #[serde(rename = "type")]
    pub sql_type: SqlType,
    pub nullable: bool,
    pub has_default: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SqlType {
    pub raw: String,
    pub normalized: String,
    pub category: SqlTypeCategory,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_type: Option<Box<SqlType>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_shape: Option<JsonShape>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SqlTypeCategory {
    String,
    Number,
    Boolean,
    Date,
    Json,
    Uuid,
    #[serde(rename = "binary")]
    Binary,
    Enum,
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QueryDef {
    pub name: String,
    pub command: QueryCommand,
    pub sql: String,
    pub params: Vec<ParamDef>,
    pub returns: Vec<ColumnDef>,
    pub source_file: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum QueryCommand {
    One,
    Many,
    Exec,
    #[serde(rename = "execresult")]
    ExecResult,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ParamDef {
    pub index: u32,
    pub name: String,
    #[serde(rename = "type")]
    pub sql_type: SqlType,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EnumDef {
    pub name: String,
    pub values: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum JsonShape {
    Object { fields: HashMap<String, JsonShape> },
    Array { element: Box<JsonShape> },
    String,
    Number,
    Boolean,
    Nullable { inner: Box<JsonShape> },
}

/// Type override map from config (e.g., "uuid" → "string")
pub type Overrides = HashMap<String, String>;
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core
```
Expected: PASS — all 5 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/sqlcx-core/src/ir.rs
git commit -m "feat(rust): add IR type definitions with serde serialization"
```

---

### Task 3: Error Types

**Branch:** `feat/rust-03-error-types` (from `main` after Task 2 merged)
**Depends on:** Task 2
**Files:**
- Create: `sqlcx-rust/crates/sqlcx-core/src/error.rs`
- Modify: `sqlcx-rust/crates/sqlcx-core/src/lib.rs`

- [ ] **Step 1: Write error types**

Write `sqlcx-rust/crates/sqlcx-core/src/error.rs`:
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SqlcxError {
    #[error("config file not found: {0}")]
    ConfigNotFound(String),

    #[error("invalid config: {0}")]
    ConfigInvalid(String),

    #[error("SQL parse error in {file}: {message}")]
    ParseError { file: String, message: String },

    #[error("unknown column type: {0}")]
    UnknownType(String),

    #[error("missing query annotation in {file}")]
    MissingAnnotation { file: String },

    #[error("unknown parser: {0}")]
    UnknownParser(String),

    #[error("unknown language: {0}")]
    UnknownLanguage(String),

    #[error("unknown schema generator: {0}")]
    UnknownSchema(String),

    #[error("unknown driver generator: {0}")]
    UnknownDriver(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
}

pub type Result<T> = std::result::Result<T, SqlcxError>;
```

- [ ] **Step 2: Verify it compiles**

```bash
cd sqlcx-rust && cargo build -p sqlcx-core
```
Expected: successful build.

- [ ] **Step 3: Commit**

```bash
git add crates/sqlcx-core/src/error.rs crates/sqlcx-core/src/lib.rs
git commit -m "feat(rust): add structured error types via thiserror"
```

---

### Task 4: Annotation Pre-processor

**Branch:** `feat/rust-04-annotations` (from `main` after Task 3 merged)
**Depends on:** Task 3
**Files:**
- Create: `sqlcx-rust/crates/sqlcx-core/src/annotations.rs`
- Modify: `sqlcx-rust/crates/sqlcx-core/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Write tests at the bottom of `sqlcx-rust/crates/sqlcx-core/src/annotations.rs`:
```rust
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
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core annotations
```
Expected: FAIL — `extract_annotations` not defined.

- [ ] **Step 3: Write annotation extractor implementation**

Write the implementation in `sqlcx-rust/crates/sqlcx-core/src/annotations.rs` above the tests.

The implementation must handle:
1. `-- name: QueryName :command` — extract query header, strip from output
2. `-- @param $N param_name` — extract param overrides, strip from output
3. `-- @enum("val1", "val2")` — extract enum values, associate with next column name
4. `-- @json({ key: type })` — parse JSON shape recursively, associate with next column name
5. All annotation lines are stripped from the cleaned SQL output
6. Regular comment lines are preserved

Key functions:
- `extract_annotations(sql: &str) -> (String, Annotations)` — main entry point
- `parse_enum_values(s: &str) -> Vec<String>` — regex-based extraction of quoted strings
- `find_next_column_name(lines: &[&str], start: usize) -> Option<String>` — look ahead for column
- `parse_json_shape(input: &str) -> Option<JsonShape>` — recursive descent parser

Reference: The TS parser in `src/parser/postgres.ts` has the annotation parsing inline — port the regex patterns and logic.

- [ ] **Step 4: Update lib.rs**

Add `pub mod annotations;` to `sqlcx-rust/crates/sqlcx-core/src/lib.rs`.

- [ ] **Step 5: Run tests to verify they pass**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core annotations
```
Expected: PASS — all 5 tests.

- [ ] **Step 6: Commit**

```bash
git add crates/sqlcx-core/src/annotations.rs crates/sqlcx-core/src/lib.rs
git commit -m "feat(rust): add annotation pre-processor for @enum/@json/@param"
```

---

### Task 5: Parameter Name Inference

**Branch:** `feat/rust-05-param-naming` (from `main` after Task 4 merged)
**Depends on:** Task 4
**Files:**
- Create: `sqlcx-rust/crates/sqlcx-core/src/param_naming.rs`
- Modify: `sqlcx-rust/crates/sqlcx-core/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Port tests from the TS `param-naming.test.ts`. Write tests at the bottom of `sqlcx-rust/crates/sqlcx-core/src/param_naming.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_column_name() {
        let params = vec![RawParam { index: 1, column: Some("id".to_string()), r#override: None }];
        assert_eq!(resolve_param_names(&params), vec!["id"]);
    }

    #[test]
    fn collision_adds_suffix() {
        let params = vec![
            RawParam { index: 1, column: Some("created_at".to_string()), r#override: None },
            RawParam { index: 2, column: Some("created_at".to_string()), r#override: None },
        ];
        assert_eq!(resolve_param_names(&params), vec!["created_at_1", "created_at_2"]);
    }

    #[test]
    fn null_column_falls_back() {
        let params = vec![RawParam { index: 1, column: None, r#override: None }];
        assert_eq!(resolve_param_names(&params), vec!["param_1"]);
    }

    #[test]
    fn override_takes_precedence() {
        let params = vec![RawParam { index: 1, column: Some("created_at".to_string()), r#override: Some("start_date".to_string()) }];
        assert_eq!(resolve_param_names(&params), vec!["start_date"]);
    }

    #[test]
    fn dedup_override_vs_inferred() {
        let params = vec![
            RawParam { index: 1, column: Some("id".to_string()), r#override: Some("id".to_string()) },
            RawParam { index: 2, column: Some("id".to_string()), r#override: None },
        ];
        let result = resolve_param_names(&params);
        assert_eq!(result[0], "id");
        assert_eq!(result[1], "id_1");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core param_naming
```
Expected: FAIL.

- [ ] **Step 3: Write parameter naming implementation**

Write `sqlcx-rust/crates/sqlcx-core/src/param_naming.rs` above the tests.

Port the exact logic from `src/parser/param-naming.ts`:
1. Pass 1: count column frequency (detect collisions)
2. Pass 2: assign names — override wins, then column name (with suffix if collision), then fallback `param_N`
3. Dedup pass: if any names still collide (override vs inferred), add `_N` suffix

- [ ] **Step 4: Update lib.rs**

Add `pub mod param_naming;` to `sqlcx-rust/crates/sqlcx-core/src/lib.rs`.

- [ ] **Step 5: Run tests to verify they pass**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core param_naming
```
Expected: PASS — all 5 tests.

- [ ] **Step 6: Commit**

```bash
git add crates/sqlcx-core/src/param_naming.rs crates/sqlcx-core/src/lib.rs
git commit -m "feat(rust): add parameter name inference with collision handling"
```

---

### Task 6: PostgreSQL Parser

**Branch:** `feat/rust-06-postgres-parser` (from `main` after Task 5 merged)
**Depends on:** Task 5
**Files:**
- Create: `sqlcx-rust/crates/sqlcx-core/src/parser/mod.rs`
- Create: `sqlcx-rust/crates/sqlcx-core/src/parser/postgres.rs`
- Modify: `sqlcx-rust/crates/sqlcx-core/src/lib.rs`

This is the **highest-risk task** — depends on `sqlparser-rs` handling our SQL patterns correctly.

- [ ] **Step 1: Write the DatabaseParser trait**

Write `sqlcx-rust/crates/sqlcx-core/src/parser/mod.rs`:
```rust
pub mod postgres;

use crate::error::Result;
use crate::ir::{EnumDef, QueryDef, TableDef};

pub trait DatabaseParser {
    fn parse_schema(&self, sql: &str) -> Result<(Vec<TableDef>, Vec<EnumDef>)>;
    fn parse_queries(
        &self,
        sql: &str,
        tables: &[TableDef],
        enums: &[EnumDef],
        source_file: &str,
    ) -> Result<Vec<QueryDef>>;
}

pub fn resolve_parser(name: &str) -> Result<Box<dyn DatabaseParser>> {
    match name {
        "postgres" => Ok(Box::new(postgres::PostgresParser::new())),
        _ => Err(crate::error::SqlcxError::UnknownParser(name.to_string())),
    }
}
```

- [ ] **Step 2: Write the failing parser tests**

Write tests at the bottom of `sqlcx-rust/crates/sqlcx-core/src/parser/postgres.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DatabaseParser;
    use crate::ir::SqlTypeCategory;

    const SCHEMA_SQL: &str = include_str!("../../../../tests/fixtures/schema.sql");
    const QUERIES_SQL: &str = include_str!("../../../../tests/fixtures/queries/users.sql");

    #[test]
    fn parses_enum_type() {
        let parser = PostgresParser::new();
        let (_, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        assert_eq!(enums.len(), 1);
        assert_eq!(enums[0].name, "user_status");
        assert_eq!(enums[0].values, vec!["active", "inactive", "banned"]);
    }

    #[test]
    fn parses_users_table() {
        let parser = PostgresParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        assert_eq!(users.columns.len(), 7);
        assert_eq!(users.primary_key, vec!["id"]);

        let id_col = &users.columns[0];
        assert_eq!(id_col.name, "id");
        assert_eq!(id_col.sql_type.category, SqlTypeCategory::Number);
        assert!(id_col.has_default);
        assert!(!id_col.nullable);

        let bio_col = users.columns.iter().find(|c| c.name == "bio").unwrap();
        assert!(bio_col.nullable);

        let tags_col = users.columns.iter().find(|c| c.name == "tags").unwrap();
        assert!(tags_col.sql_type.element_type.is_some());
    }

    #[test]
    fn parses_posts_table() {
        let parser = PostgresParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let posts = tables.iter().find(|t| t.name == "posts").unwrap();
        assert_eq!(posts.columns.len(), 6);
    }

    #[test]
    fn parses_get_user_query() {
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser.parse_queries(QUERIES_SQL, &tables, &enums, "queries/users.sql").unwrap();

        let get_user = queries.iter().find(|q| q.name == "GetUser").unwrap();
        assert_eq!(get_user.command, QueryCommand::One);
        assert_eq!(get_user.params.len(), 1);
        assert_eq!(get_user.params[0].name, "id");
        // SELECT * returns all users columns
        assert_eq!(get_user.returns.len(), 7);
    }

    #[test]
    fn parses_list_users_partial_select() {
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser.parse_queries(QUERIES_SQL, &tables, &enums, "queries/users.sql").unwrap();

        let list_users = queries.iter().find(|q| q.name == "ListUsers").unwrap();
        assert_eq!(list_users.command, QueryCommand::Many);
        // SELECT id, name, email — only 3 columns
        assert_eq!(list_users.returns.len(), 3);
    }

    #[test]
    fn parses_create_user_exec() {
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser.parse_queries(QUERIES_SQL, &tables, &enums, "queries/users.sql").unwrap();

        let create_user = queries.iter().find(|q| q.name == "CreateUser").unwrap();
        assert_eq!(create_user.command, QueryCommand::Exec);
        assert_eq!(create_user.params.len(), 3);
        assert!(create_user.returns.is_empty());
    }

    #[test]
    fn parses_delete_user_execresult() {
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser.parse_queries(QUERIES_SQL, &tables, &enums, "queries/users.sql").unwrap();

        let delete_user = queries.iter().find(|q| q.name == "DeleteUser").unwrap();
        assert_eq!(delete_user.command, QueryCommand::ExecResult);
    }

    #[test]
    fn parses_param_overrides() {
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser.parse_queries(QUERIES_SQL, &tables, &enums, "queries/users.sql").unwrap();

        let date_range = queries.iter().find(|q| q.name == "ListUsersByDateRange").unwrap();
        assert_eq!(date_range.params[0].name, "start_date");
        assert_eq!(date_range.params[1].name, "end_date");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core parser
```
Expected: FAIL — `PostgresParser` not defined.

- [ ] **Step 4: Write PostgreSQL parser implementation**

Write `sqlcx-rust/crates/sqlcx-core/src/parser/postgres.rs` above the tests. This is the largest file (~300 lines).

**The subagent MUST read `src/parser/postgres.ts` to match exact behavior.** Key implementation points:

1. **Enum parsing:** Regex `CREATE TYPE name AS ENUM ('a', 'b', 'c')` — `sqlparser-rs` may or may not handle this, so use regex as fallback (TS version uses regex)
2. **Table parsing:** Use `sqlparser-rs` `Statement::CreateTable` AST node. Walk columns to extract name, type, nullable (absence of `NOT NULL`), default (presence of `DEFAULT`), primary key
3. **Type mapping:** Match raw SQL type strings to `SqlTypeCategory`:
   - `TEXT|VARCHAR|CHAR|CHARACTER VARYING` → String
   - `INT|INTEGER|SERIAL|BIGINT|SMALLINT|BIGSERIAL|REAL|FLOAT|DOUBLE|NUMERIC|DECIMAL` → Number
   - `BOOLEAN|BOOL` → Boolean
   - `TIMESTAMP|TIMESTAMPTZ|DATE|TIME|TIMETZ` → Date
   - `JSON|JSONB` → Json
   - `UUID` → Uuid
   - `BYTEA` → Binary
   - Named type that matches an enum → Enum
4. **Array types:** `TEXT[]` → `element_type` with String category
5. **Query splitting:** Split query file by `-- name:` header lines into individual query blocks
6. **Query parsing:** For each block, use `extract_annotations()` then `sqlparser-rs` to parse the statement
7. **SELECT * expansion:** When `SELECT *`, look up the table in `tables` and return all columns
8. **Partial SELECT:** Match column names against table definitions
9. **INSERT params:** Infer from column list in `INSERT INTO table (col1, col2) VALUES ($1, $2)`
10. **WHERE params:** Infer from `WHERE col = $1` patterns
11. **Parameter naming:** Collect `RawParam`s and call `resolve_param_names()`

- [ ] **Step 5: Update lib.rs**

Add `pub mod parser;` to `sqlcx-rust/crates/sqlcx-core/src/lib.rs`.

- [ ] **Step 6: Run tests to verify they pass**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core parser
```
Expected: PASS — all 8 tests.

- [ ] **Step 7: Commit**

```bash
git add crates/sqlcx-core/src/parser/ crates/sqlcx-core/src/lib.rs
git commit -m "feat(rust): add PostgreSQL parser via sqlparser-rs"
```

---

### Task 7: Generator Traits

**Branch:** `feat/rust-07-generator-traits` (from `main` after Task 6 merged)
**Depends on:** Task 6
**Files:**
- Create: `sqlcx-rust/crates/sqlcx-core/src/generator/mod.rs`
- Create: `sqlcx-rust/crates/sqlcx-core/src/generator/typescript/mod.rs`
- Create: `sqlcx-rust/crates/sqlcx-core/src/generator/typescript/typebox.rs` (placeholder)
- Create: `sqlcx-rust/crates/sqlcx-core/src/generator/typescript/bun_sql.rs` (placeholder)
- Create: `sqlcx-rust/crates/sqlcx-core/src/config.rs`
- Modify: `sqlcx-rust/crates/sqlcx-core/src/lib.rs`

- [ ] **Step 1: Write generator trait definitions and config structs**

Write `sqlcx-rust/crates/sqlcx-core/src/generator/mod.rs` with the trait definitions: `SchemaGenerator`, `DriverGenerator`, `LanguagePlugin`, `GeneratedFile`, and `resolve_language()`.

Write `sqlcx-rust/crates/sqlcx-core/src/config.rs` with `SqlcxConfig` and `TargetConfig` structs (both `Deserialize`). Include `load_config()` function stub.

Write placeholder files for `typescript/mod.rs`, `typescript/typebox.rs`, `typescript/bun_sql.rs`.

Reference: Task 7 in the detailed plan above for exact code.

- [ ] **Step 2: Update lib.rs**

Add `pub mod generator;` and `pub mod config;` to lib.rs.

- [ ] **Step 3: Verify it compiles**

```bash
cd sqlcx-rust && cargo build -p sqlcx-core
```
Expected: successful build.

- [ ] **Step 4: Commit**

```bash
git add crates/sqlcx-core/src/generator/ crates/sqlcx-core/src/config.rs crates/sqlcx-core/src/lib.rs
git commit -m "feat(rust): add generator trait definitions and plugin registry"
```

---

### Task 8: TypeBox Schema Generator

**Branch:** `feat/rust-08-typebox-generator` (from `main` after Task 7 merged)
**Depends on:** Task 7
**Can parallel with:** Task 9
**Files:**
- Modify: `sqlcx-rust/crates/sqlcx-core/src/generator/typescript/typebox.rs`

- [ ] **Step 1: Write the failing snapshot test**

Write snapshot tests that parse the fixture schema into IR, then run the TypeBox generator and snapshot the output.

- [ ] **Step 2: Run test to verify it fails**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core typebox
```
Expected: FAIL.

- [ ] **Step 3: Write TypeBox generator implementation**

Port the logic from `src/generator/typescript/schema/typebox.ts`. The subagent MUST read that file to match exact output.

Key functions to implement:
- `type_box_type(sql_type: &SqlType) -> String` — maps SqlType to TypeBox call
- `select_column(col: &ColumnDef) -> String` — wraps nullable with `Type.Union([..., Type.Null()])`
- `insert_column(col: &ColumnDef) -> String` — adds `Type.Optional(...)` for defaults/nullable
- `generate_imports() -> String` — TypeBox import line + Prettify type
- `generate_enum_schema(enum_def: &EnumDef) -> String`
- `generate_select_schema(table: &TableDef) -> String`
- `generate_insert_schema(table: &TableDef) -> String`
- `generate_type_alias(name: &str, schema_var: &str) -> String`

Use `pascal_case()` helper for type names. Use `write!`/`writeln!` for string building.

- [ ] **Step 4: Run test and review snapshot**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core typebox && cargo insta review
```

- [ ] **Step 5: Commit**

```bash
git add crates/sqlcx-core/src/generator/typescript/typebox.rs
git commit -m "feat(rust): add TypeBox schema generator"
```

---

### Task 9: Bun.sql Driver Generator

**Branch:** `feat/rust-09-bun-sql-generator` (from `main` after Task 7 merged)
**Depends on:** Task 7
**Can parallel with:** Task 8
**Files:**
- Modify: `sqlcx-rust/crates/sqlcx-core/src/generator/typescript/bun_sql.rs`

- [ ] **Step 1: Write the failing snapshot test**

Write snapshot tests that parse full fixtures (schema + queries) into IR, then run the Bun.sql generator and snapshot client + query files.

- [ ] **Step 2: Run test to verify it fails**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core bun_sql
```
Expected: FAIL.

- [ ] **Step 3: Write Bun.sql driver generator implementation**

Port the logic from `src/generator/typescript/driver/bun-sql.ts`. The subagent MUST read that file to match exact output.

Key functions:
- `ts_type(sql_type: &SqlType) -> String` — maps SqlType to TypeScript type
- `generate_client() -> String` — DatabaseClient interface + BunSqlClient class
- `generate_row_type(query: &QueryDef) -> String` — row interface
- `generate_params_type(query: &QueryDef) -> String` — params interface
- `generate_query_function(query: &QueryDef) -> String` — typed async function

Important: use `split_words()` for PascalCase/camelCase conversion of query names (same as TS `splitWords`).

- [ ] **Step 4: Run test and review snapshot**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core bun_sql && cargo insta review
```

- [ ] **Step 5: Commit**

```bash
git add crates/sqlcx-core/src/generator/typescript/bun_sql.rs
git commit -m "feat(rust): add Bun.sql driver generator"
```

---

### Task 10: TypeScript Plugin Orchestrator

**Branch:** `feat/rust-10-typescript-plugin` (from `main` after Tasks 8+9 merged)
**Depends on:** Tasks 8 and 9
**Files:**
- Modify: `sqlcx-rust/crates/sqlcx-core/src/generator/typescript/mod.rs`

- [ ] **Step 1: Write the failing test**

Test that the TypeScript plugin generates exactly 3 files (schema.ts, client.ts, users.queries.ts) from the fixture IR.

- [ ] **Step 2: Run test to verify it fails**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core typescript::tests
```
Expected: FAIL.

- [ ] **Step 3: Implement TypeScript plugin orchestrator**

Port from `src/generator/typescript/index.ts`. Orchestration order:
1. Generate schema.ts via TypeBoxGenerator (imports → enums → select/insert schemas → type aliases for tables → type aliases for enums)
2. Generate client.ts via BunSqlGenerator (DatabaseClient interface → adapter class)
3. Generate query files: group queries by source_file, one `.queries.ts` per group (import DatabaseClient → query functions)

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core typescript
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sqlcx-core/src/generator/typescript/mod.rs
git commit -m "feat(rust): add TypeScript plugin orchestrator"
```

---

### Task 11: Config Loading

**Branch:** `feat/rust-11-config` (from `main` after Task 7 merged)
**Depends on:** Task 7 (for config struct definitions)
**Can parallel with:** Task 12
**Files:**
- Modify: `sqlcx-rust/crates/sqlcx-core/src/config.rs`

- [ ] **Step 1: Write the failing tests**

Test TOML deserialization, JSON deserialization, auto-detection (TOML first, then JSON), and config-not-found error.

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core config
```
Expected: FAIL.

- [ ] **Step 3: Implement load_config**

Add `load_config(dir: &Path) -> Result<SqlcxConfig>` that auto-detects `sqlcx.toml` (tried first) or `sqlcx.json`.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core config
```
Expected: PASS — all tests.

- [ ] **Step 5: Commit**

```bash
git add crates/sqlcx-core/src/config.rs
git commit -m "feat(rust): add TOML/JSON config loading with auto-detection"
```

---

### Task 12: IR Cache

**Branch:** `feat/rust-12-cache` (from `main` after Task 7 merged)
**Depends on:** Task 7 (for IR types)
**Can parallel with:** Task 11
**Files:**
- Create: `sqlcx-rust/crates/sqlcx-core/src/cache.rs`
- Modify: `sqlcx-rust/crates/sqlcx-core/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Test: hash determinism, hash order-independence, cache round-trip, hash mismatch = miss, no file = miss.

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core cache
```
Expected: FAIL.

- [ ] **Step 3: Write cache implementation**

Port from `src/cache/index.ts`:
- `compute_hash(files: &[SqlFile]) -> String` — SHA-256 of sorted path+content with null separators
- `write_cache(cache_dir, ir, hash)` — atomic write (temp file → rename)
- `read_cache(cache_dir, expected_hash)` — load, verify hash, return IR or None

- [ ] **Step 4: Update lib.rs**

Add `pub mod cache;`.

- [ ] **Step 5: Run tests to verify they pass**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core cache
```
Expected: PASS — all 5 tests.

- [ ] **Step 6: Commit**

```bash
git add crates/sqlcx-core/src/cache.rs crates/sqlcx-core/src/lib.rs
git commit -m "feat(rust): add SHA-256 IR caching with atomic writes"
```

---

### Task 13: CLI

**Branch:** `feat/rust-13-cli` (from `main` after Task 12 merged)
**Depends on:** Task 12
**Files:**
- Modify: `sqlcx-rust/crates/sqlcx/src/main.rs`
- Create: `sqlcx-rust/tests/cli.rs`

- [ ] **Step 1: Write CLI integration tests**

Test: `--help` output, `generate` with fixtures produces 3 files, `check` validates without writing.

- [ ] **Step 2: Write CLI implementation**

Wire up clap with 4 subcommands: `generate`, `check`, `init`, `schema`.

`generate` and `check` share a pipeline:
1. Load config from cwd
2. Glob SQL files from `config.sql` directory
3. Compute hash, check cache
4. Parse schema files → tables + enums
5. Parse query files → queries
6. Build IR, write cache
7. For each target: resolve language plugin, generate files, write to disk (generate only)

`schema` outputs JSON Schema for config validation to stdout using `schemars::schema_for!(SqlcxConfig)`.
`init` is a stub for now (print TODO).

Note: Config structs in `config.rs` must derive `schemars::JsonSchema` in addition to `Deserialize`.

- [ ] **Step 3: Run CLI tests**

```bash
cd sqlcx-rust && cargo test --test cli
```
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/sqlcx/src/main.rs tests/cli.rs
git commit -m "feat(rust): add CLI with generate, check, init, schema commands"
```

---

### Task 14: End-to-End Snapshot Tests

**Branch:** `feat/rust-14-e2e-tests` (from `main` after Task 13 merged)
**Depends on:** Task 13
**Files:**
- Create: `sqlcx-rust/tests/e2e.rs`

- [ ] **Step 1: Write E2E snapshot tests**

Full pipeline test: fixture SQL → parser → IR → TypeScript plugin → snapshot each generated file.
Also test that IR JSON uses camelCase keys matching the TS version.

- [ ] **Step 2: Run E2E tests and review snapshots**

```bash
cd sqlcx-rust && cargo test --test e2e && cargo insta review
```

Compare generated TypeScript output against what the TS version produces by running `bun run generate` in the project root.

- [ ] **Step 3: Commit**

```bash
git add tests/e2e.rs
git commit -m "feat(rust): add end-to-end snapshot tests for full pipeline"
```

---

### Task 15: npm Packaging

**Branch:** `feat/rust-15-npm-packaging` (from `main` after Task 14 merged)
**Depends on:** Task 14
**Files:**
- Create: `sqlcx-rust/npm/sqlcx/package.json`
- Create: `sqlcx-rust/npm/sqlcx/bin/sqlcx` (JS shim)
- Create: `sqlcx-rust/npm/darwin-arm64/package.json`
- Create: `sqlcx-rust/npm/darwin-x64/package.json`
- Create: `sqlcx-rust/npm/linux-x64-gnu/package.json`
- Create: `sqlcx-rust/npm/linux-arm64-gnu/package.json`
- Create: `sqlcx-rust/npm/win32-x64-msvc/package.json`

- [ ] **Step 1: Create main npm package**

`package.json` with `name: "sqlcx-orm"`, bin entry, and `optionalDependencies` for all 5 platform packages.

- [ ] **Step 2: Create JS shim**

Node.js script that detects `os.platform()` + `os.arch()`, resolves the platform package, and spawns the binary with `execFileSync` (safe — no shell interpolation).

- [ ] **Step 3: Create platform package templates**

One `package.json` per platform with correct `os` and `cpu` fields.

- [ ] **Step 4: Verify package structure**

```bash
ls -la sqlcx-rust/npm/sqlcx/ sqlcx-rust/npm/darwin-arm64/
```

- [ ] **Step 5: Commit**

```bash
git add sqlcx-rust/npm/
git commit -m "feat(rust): add npm packaging with platform-specific optional dependencies"
```

---

## Summary

| Task | Component | Est. Complexity | Parallelizable |
|------|-----------|----------------|----------------|
| 1 | Workspace scaffolding | Low | No |
| 2 | IR types | Low | No |
| 3 | Error types | Low | No |
| 4 | Annotations | Medium | No |
| 5 | Param naming | Low | No |
| 6 | PostgreSQL parser | High | No |
| 7 | Generator traits | Low | No |
| 8 | TypeBox generator | Medium | Yes (with 9) |
| 9 | Bun.sql driver | Medium | Yes (with 8) |
| 10 | TS plugin orchestrator | Medium | No |
| 11 | Config loading | Low | Yes (with 12) |
| 12 | IR cache | Low | Yes (with 11) |
| 13 | CLI | Medium | No |
| 14 | E2E tests | Low | No |
| 15 | npm packaging | Low | No |

**Critical path:** Tasks 1-7 → 8+9 (parallel) + 11+12 (parallel) → 10 → 13-15
**Highest risk:** Task 6 (PostgreSQL parser) — depends on how well `sqlparser-rs` handles our use cases.
