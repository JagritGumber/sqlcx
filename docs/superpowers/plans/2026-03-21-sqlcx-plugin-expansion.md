# sqlcx Plugin Expansion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand sqlcx from 1 database / 1 schema / 1 driver to 2 databases / 3 schemas / 2 drivers, with proper trait dispatch and TypeScript type-check verification.

**Architecture:** Refactor the existing hard-coded plugin wiring to use trait-based dispatch via resolver functions, extract shared utilities, then add MySQL parser, Zod v4/v3 schema generators, pg driver generator, and tsc verification infrastructure — all in parallel where possible.

**Tech Stack:** Rust, serde, regex, insta (snapshots), npm/tsc (type verification)

**Spec:** `docs/superpowers/specs/2026-03-21-sqlcx-plugin-expansion-design.md`
**Base Spec:** `docs/superpowers/specs/2026-03-21-sqlcx-rust-rewrite-design.md`

---

## Branch Strategy

```
main ←── feat/rust-16-shared-utils
     ←── feat/rust-17-refactor-trait-dispatch
     ←── feat/rust-18-mysql-parser              (can parallel with 19, 20, 21)
     ←── feat/rust-19-zod-v4-generator          (can parallel with 18, 20, 21)
     ←── feat/rust-20-zod-v3-generator          (can parallel with 18, 19, 21)
     ←── feat/rust-21-pg-driver                 (can parallel with 18, 19, 20)
     ←── feat/rust-22-typecheck-infra
```

**Parallelizable:** Tasks 18-21 are fully independent (each implements a trait, no shared code). They all depend on Tasks 16+17 being merged first.

---

## File Map

```
crates/sqlcx-core/src/
├── utils.rs                                    # NEW (Task 16): shared helpers
├── lib.rs                                      # MODIFIED (Task 16): add pub mod utils
├── parser/
│   ├── mod.rs                                  # MODIFIED (Task 18): add mysql match arm
│   ├── postgres.rs                             # EXISTING (unchanged)
│   └── mysql.rs                                # NEW (Task 18): MySQL parser
├── generator/
│   ├── mod.rs                                  # MODIFIED (Task 17): update resolve_language
│   └── typescript/
│       ├── mod.rs                              # MODIFIED (Task 17): add resolve_schema/resolve_driver, trait dispatch
│       ├── typebox.rs                          # MODIFIED (Tasks 16, 17): use crate::utils, impl SchemaGenerator
│       ├── bun_sql.rs                          # MODIFIED (Tasks 16, 17): use crate::utils, impl DriverGenerator
│       ├── zod.rs                              # NEW (Task 19): Zod v4 schema generator
│       ├── zod_v3.rs                           # NEW (Task 20): Zod v3 schema generator
│       └── pg.rs                               # NEW (Task 21): pg driver generator
├── config.rs                                   # MODIFIED (Task 17): add overrides access

tests/
├── fixtures/
│   ├── schema.sql                              # EXISTING (Postgres)
│   ├── queries/users.sql                       # EXISTING (Postgres)
│   ├── mysql_schema.sql                        # NEW (Task 18)
│   └── mysql_queries/users.sql                 # NEW (Task 18)
├── typecheck/                                  # NEW (Task 22)
│   ├── package.json
│   ├── tsconfig.json
│   └── generated/                              # gitignored
```

---

### Task 16: Shared Utilities Extraction

**Branch:** `feat/rust-16-shared-utils` (from `main`)
**Depends on:** nothing
**Files:**
- Create: `sqlcx-rust/crates/sqlcx-core/src/utils.rs`
- Modify: `sqlcx-rust/crates/sqlcx-core/src/lib.rs`
- Modify: `sqlcx-rust/crates/sqlcx-core/src/generator/typescript/typebox.rs`
- Modify: `sqlcx-rust/crates/sqlcx-core/src/generator/typescript/bun_sql.rs`

- [ ] **Step 1: Create utils.rs with tests**

Create `sqlcx-rust/crates/sqlcx-core/src/utils.rs` by extracting and deduplicating:
- `pascal_case()` from `typebox.rs:10` (snake_case → PascalCase)
- `escape_string()` from `typebox.rs:25` (JSON.stringify-style escaping)
- `split_words()` from `bun_sql.rs:10` (insert underscores between camelCase words)
- `camel_case()` from `bun_sql.rs:36` (snake_case → camelCase)

All functions must be `pub`. Add tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_case_snake() { assert_eq!(pascal_case("user_status"), "UserStatus"); }
    #[test]
    fn pascal_case_single() { assert_eq!(pascal_case("users"), "Users"); }
    #[test]
    fn camel_case_snake() { assert_eq!(camel_case("get_user"), "getUser"); }
    #[test]
    fn split_words_pascal() { assert_eq!(split_words("GetUser"), "Get_User"); }
    #[test]
    fn escape_string_quotes() { assert_eq!(escape_string("he said \"hi\""), "he said \\\"hi\\\""); }
}
```

- [ ] **Step 2: Update lib.rs**

Add `pub mod utils;` to `sqlcx-rust/crates/sqlcx-core/src/lib.rs`.

- [ ] **Step 3: Update typebox.rs to use crate::utils**

In `typebox.rs`:
- Remove `pub fn pascal_case()` and `pub fn escape_string()` function definitions
- Add `use crate::utils::{pascal_case, escape_string};` at the top

- [ ] **Step 4: Update bun_sql.rs to use crate::utils**

In `bun_sql.rs`:
- Remove `fn split_words()`, `fn pascal_case()`, `fn camel_case()` function definitions
- Add `use crate::utils::{pascal_case, camel_case, split_words, escape_string};` at the top
- Note: `escape_string` may not be used in bun_sql.rs yet — only import what's needed. The `json_stringify` function in bun_sql.rs may be its own thing — read the file to check. If bun_sql has its own string escaping, keep it and only import the case utilities.

- [ ] **Step 5: Run all tests to verify no regressions**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core
```
Expected: all existing tests still pass (currently 58+ tests).

- [ ] **Step 6: Commit**

```bash
git add crates/sqlcx-core/src/utils.rs crates/sqlcx-core/src/lib.rs crates/sqlcx-core/src/generator/typescript/typebox.rs crates/sqlcx-core/src/generator/typescript/bun_sql.rs
git commit -m "refactor(rust): extract shared utils (pascal_case, camel_case, etc.)"
```

---

### Task 17: Refactor to Trait-Based Dispatch

**Branch:** `feat/rust-17-refactor-trait-dispatch` (from `main` after Task 16)
**Depends on:** Task 16
**Files:**
- Modify: `sqlcx-rust/crates/sqlcx-core/src/generator/typescript/mod.rs`
- Modify: `sqlcx-rust/crates/sqlcx-core/src/generator/typescript/typebox.rs`
- Modify: `sqlcx-rust/crates/sqlcx-core/src/generator/typescript/bun_sql.rs`
- Modify: `sqlcx-rust/crates/sqlcx-core/src/generator/mod.rs`
- Modify: `sqlcx-rust/crates/sqlcx-core/src/config.rs`

This task refactors the existing hard-coded plugin wiring to use trait-based dispatch. **No new functionality** — just restructuring so new plugins can be added.

- [ ] **Step 1: Make TypeBoxGenerator implement SchemaGenerator trait**

In `typebox.rs`, the `TypeBoxGenerator` currently has an ad-hoc `generate_schema_file()` method. Refactor it to implement the `SchemaGenerator` trait:

```rust
impl SchemaGenerator for TypeBoxGenerator {
    fn generate(&self, ir: &SqlcxIR, overrides: &Overrides) -> Result<GeneratedFile> {
        Ok(GeneratedFile {
            path: "schema.ts".to_string(),
            content: self.generate_schema_file(ir, overrides),
        })
    }
}
```

Keep the existing `generate_schema_file()` as a private helper. The trait impl wraps it.

- [ ] **Step 2: Make BunSqlGenerator implement DriverGenerator trait**

In `bun_sql.rs`, the `BunSqlGenerator` currently has ad-hoc `generate_client()` and `generate_query_functions()` methods. Implement the `DriverGenerator` trait:

```rust
impl DriverGenerator for BunSqlGenerator {
    fn generate(&self, ir: &SqlcxIR) -> Result<Vec<GeneratedFile>> {
        let mut files = Vec::new();

        // 1. client.ts
        files.push(GeneratedFile {
            path: "client.ts".to_string(),
            content: self.generate_client(),
        });

        // 2. Group queries by source_file → one .queries.ts per file
        let mut grouped: HashMap<String, Vec<&QueryDef>> = HashMap::new();
        for query in &ir.queries {
            grouped.entry(query.source_file.clone()).or_default().push(query);
        }
        for (source_file, queries) in &grouped {
            let basename = std::path::Path::new(source_file)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let owned: Vec<QueryDef> = queries.iter().map(|q| (*q).clone()).collect();
            files.push(GeneratedFile {
                path: format!("{}.queries.ts", basename),
                content: self.generate_query_functions(&owned),
            });
        }

        Ok(files)
    }
}
```

This moves the query grouping logic from `typescript/mod.rs` into the driver, where it belongs.

- [ ] **Step 3: Add resolve_schema and resolve_driver to typescript/mod.rs**

```rust
use crate::error::{Result, SqlcxError};

fn resolve_schema(name: &str) -> Result<Box<dyn SchemaGenerator>> {
    match name {
        "typebox" => Ok(Box::new(typebox::TypeBoxGenerator)),
        _ => Err(SqlcxError::UnknownSchema(name.to_string())),
    }
}

fn resolve_driver(name: &str) -> Result<Box<dyn DriverGenerator>> {
    match name {
        "bun-sql" => Ok(Box::new(bun_sql::BunSqlGenerator)),
        _ => Err(SqlcxError::UnknownDriver(name.to_string())),
    }
}
```

- [ ] **Step 4: Add overrides field to TargetConfig and wire through**

In `config.rs`, add `overrides` to `TargetConfig`:
```rust
#[derive(Deserialize, Clone, Debug)]
pub struct TargetConfig {
    pub language: String,
    pub out: String,
    pub schema: String,
    pub driver: String,
    #[serde(default)]
    pub overrides: HashMap<String, String>,
}
```

In `main.rs`, before calling `plugin.generate()`, merge top-level overrides into target overrides (target-level wins):
```rust
for target in &config.targets {
    let mut merged_target = target.clone();
    for (k, v) in &config.overrides {
        merged_target.overrides.entry(k.clone()).or_insert(v.clone());
    }
    let plugin = resolve_language(&merged_target.language, &merged_target.schema, &merged_target.driver)?;
    let files = plugin.generate(&ir, &merged_target)?;
    // ... write files
}
```

Also update the existing `generates_three_files` test in `typescript/mod.rs` to include the new `overrides` field:
```rust
let config = TargetConfig {
    language: "typescript".to_string(),
    out: "./src/db".to_string(),
    schema: "typebox".to_string(),
    driver: "bun-sql".to_string(),
    overrides: HashMap::new(),
};
```

- [ ] **Step 5: Update TypeScriptPlugin::generate() to use resolvers**

Replace the hard-coded generator calls with trait dispatch:

```rust
impl LanguagePlugin for TypeScriptPlugin {
    fn generate(&self, ir: &SqlcxIR, config: &TargetConfig) -> Result<Vec<GeneratedFile>> {
        let schema_gen = resolve_schema(&self.schema_name)?;
        let driver_gen = resolve_driver(&self.driver_name)?;
        let overrides = &config.overrides;

        let mut files = Vec::new();

        // Schema file (schema.ts)
        let mut schema_file = schema_gen.generate(ir, overrides)?;
        schema_file.path = join_path(&config.out, &schema_file.path);
        files.push(schema_file);

        // Driver files (client.ts + *.queries.ts)
        let driver_files = driver_gen.generate(ir)?;
        for mut f in driver_files {
            f.path = join_path(&config.out, &f.path);
            files.push(f);
        }

        Ok(files)
    }
}
```

- [ ] **Step 6: Run all tests**

```bash
cd sqlcx-rust && cargo test
```
Expected: all existing tests pass. The `generates_three_files` test in `typescript/mod.rs` should still work since the output is identical — just the wiring changed.

- [ ] **Step 7: Commit**

```bash
git commit -m "refactor(rust): use trait-based dispatch for schema/driver plugins"
```

---

### Task 18: MySQL Parser

**Branch:** `feat/rust-18-mysql-parser` (from `main` after Task 17)
**Depends on:** Task 17
**Can parallel with:** Tasks 19, 20, 21
**Files:**
- Create: `sqlcx-rust/crates/sqlcx-core/src/parser/mysql.rs`
- Modify: `sqlcx-rust/crates/sqlcx-core/src/parser/mod.rs`
- Create: `sqlcx-rust/tests/fixtures/mysql_schema.sql`
- Create: `sqlcx-rust/tests/fixtures/mysql_queries/users.sql`

- [ ] **Step 1: Create MySQL test fixtures**

Write `sqlcx-rust/tests/fixtures/mysql_schema.sql`:
```sql
CREATE TABLE users (
  id INT AUTO_INCREMENT PRIMARY KEY,
  name VARCHAR(255) NOT NULL,
  email VARCHAR(255) NOT NULL UNIQUE,
  bio TEXT,
  role ENUM('admin', 'user', 'guest') NOT NULL DEFAULT 'user',
  preferences JSON,
  is_active TINYINT(1) NOT NULL DEFAULT 1,
  score DECIMAL(10, 2),
  avatar BLOB,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  full_name VARCHAR(255) GENERATED ALWAYS AS (CONCAT(name, ' ', email)) STORED
);

CREATE TABLE posts (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  user_id INT NOT NULL,
  title VARCHAR(255) NOT NULL,
  body LONGTEXT NOT NULL,
  published TINYINT(1) NOT NULL DEFAULT 0,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (user_id) REFERENCES users(id)
);
```

Write `sqlcx-rust/tests/fixtures/mysql_queries/users.sql`:
```sql
-- name: GetUser :one
SELECT * FROM users WHERE id = ?;

-- name: ListUsers :many
SELECT id, name, email FROM users WHERE name LIKE ?;

-- name: CreateUser :exec
INSERT INTO users (name, email, bio) VALUES (?, ?, ?);

-- name: DeleteUser :execresult
DELETE FROM users WHERE id = ?;

-- name: ListUsersByDateRange :many
-- @param $1 start_date
-- @param $2 end_date
SELECT * FROM users WHERE created_at > ? AND created_at < ?;
```

- [ ] **Step 2: Write the failing parser tests**

Write tests at the bottom of `sqlcx-rust/crates/sqlcx-core/src/parser/mysql.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::DatabaseParser;
    use crate::ir::SqlTypeCategory;

    const SCHEMA_SQL: &str = include_str!("../../../../tests/fixtures/mysql_schema.sql");
    const QUERIES_SQL: &str = include_str!("../../../../tests/fixtures/mysql_queries/users.sql");

    #[test]
    fn parses_users_table() {
        let parser = MySqlParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        assert_eq!(users.columns.len(), 11);
        assert_eq!(users.primary_key, vec!["id"]);

        let id_col = &users.columns[0];
        assert_eq!(id_col.name, "id");
        assert_eq!(id_col.sql_type.category, SqlTypeCategory::Number);
        assert!(id_col.has_default); // AUTO_INCREMENT
        assert!(!id_col.nullable);
    }

    #[test]
    fn parses_inline_enum() {
        let parser = MySqlParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        let role_col = users.columns.iter().find(|c| c.name == "role").unwrap();
        assert_eq!(role_col.sql_type.category, SqlTypeCategory::Enum);
        assert_eq!(role_col.sql_type.enum_values, Some(vec!["admin".to_string(), "user".to_string(), "guest".to_string()]));
    }

    #[test]
    fn parses_tinyint_as_boolean() {
        let parser = MySqlParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        let active_col = users.columns.iter().find(|c| c.name == "is_active").unwrap();
        assert_eq!(active_col.sql_type.category, SqlTypeCategory::Boolean);
    }

    #[test]
    fn parses_json_column() {
        let parser = MySqlParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        let prefs = users.columns.iter().find(|c| c.name == "preferences").unwrap();
        assert_eq!(prefs.sql_type.category, SqlTypeCategory::Json);
        assert!(prefs.nullable);
    }

    #[test]
    fn parses_blob_as_binary() {
        let parser = MySqlParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        let avatar = users.columns.iter().find(|c| c.name == "avatar").unwrap();
        assert_eq!(avatar.sql_type.category, SqlTypeCategory::Binary);
    }

    #[test]
    fn parses_generated_column_as_default() {
        let parser = MySqlParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        let full_name = users.columns.iter().find(|c| c.name == "full_name").unwrap();
        assert!(full_name.has_default); // GENERATED ALWAYS AS = implicit default
    }

    #[test]
    fn parses_posts_table() {
        let parser = MySqlParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let posts = tables.iter().find(|t| t.name == "posts").unwrap();
        assert_eq!(posts.columns.len(), 6);
    }

    #[test]
    fn parses_query_with_positional_params() {
        let parser = MySqlParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser.parse_queries(QUERIES_SQL, &tables, &enums, "mysql_queries/users.sql").unwrap();

        let get_user = queries.iter().find(|q| q.name == "GetUser").unwrap();
        assert_eq!(get_user.command, QueryCommand::One);
        assert_eq!(get_user.params.len(), 1);
        assert_eq!(get_user.params[0].name, "id");
        assert_eq!(get_user.returns.len(), 11);
    }

    #[test]
    fn parses_insert_params() {
        let parser = MySqlParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser.parse_queries(QUERIES_SQL, &tables, &enums, "mysql_queries/users.sql").unwrap();

        let create = queries.iter().find(|q| q.name == "CreateUser").unwrap();
        assert_eq!(create.command, QueryCommand::Exec);
        assert_eq!(create.params.len(), 3);
        assert!(create.returns.is_empty());
    }

    #[test]
    fn parses_param_overrides() {
        let parser = MySqlParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser.parse_queries(QUERIES_SQL, &tables, &enums, "mysql_queries/users.sql").unwrap();

        let date_range = queries.iter().find(|q| q.name == "ListUsersByDateRange").unwrap();
        assert_eq!(date_range.params[0].name, "start_date");
        assert_eq!(date_range.params[1].name, "end_date");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core mysql
```
Expected: FAIL — `MySqlParser` not defined.

- [ ] **Step 4: Implement MySqlParser**

Write `sqlcx-rust/crates/sqlcx-core/src/parser/mysql.rs` (above tests). Port the regex-based approach from `postgres.rs` with MySQL-specific changes:

Key differences from Postgres parser:
- No `CREATE TYPE ... AS ENUM` — enums are inline on columns: `ENUM('a','b','c')`
- Parse inline enum regex: `ENUM\s*\(([^)]+)\)` → extract quoted values
- `AUTO_INCREMENT` → `has_default = true`
- `TINYINT(1)` → `SqlTypeCategory::Boolean`
- No array types
- `?` placeholder instead of `$N` — count occurrences left-to-right
- `DEFAULT CURRENT_TIMESTAMP` → `has_default = true`

**Read `src/parser/postgres.rs` (the Rust file, not the TS one)** to follow the same patterns: regex-based table/column parsing, `extract_annotations()` integration, `resolve_param_names()` for naming.

- [ ] **Step 5: Add mysql match arm to resolver**

In `parser/mod.rs`:
```rust
pub mod mysql;

// In resolve_parser:
"mysql" => Ok(Box::new(mysql::MySqlParser::new())),
```

- [ ] **Step 6: Run tests**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core mysql
```
Expected: PASS — all 8 MySQL tests.

- [ ] **Step 7: Run all tests for regressions**

```bash
cd sqlcx-rust && cargo test
```
Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git commit -m "feat(rust): add MySQL parser with inline ENUM, AUTO_INCREMENT, TINYINT(1) boolean"
```

---

### Task 19: Zod v4 Schema Generator

**Branch:** `feat/rust-19-zod-v4-generator` (from `main` after Task 17)
**Depends on:** Task 17
**Can parallel with:** Tasks 18, 20, 21
**Files:**
- Create: `sqlcx-rust/crates/sqlcx-core/src/generator/typescript/zod.rs`
- Modify: `sqlcx-rust/crates/sqlcx-core/src/generator/typescript/mod.rs`

- [ ] **Step 1: Write failing snapshot test**

At the bottom of `zod.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;
    use crate::parser::postgres::PostgresParser;
    use crate::parser::DatabaseParser;
    use std::collections::HashMap;

    fn parse_fixture_ir() -> SqlcxIR {
        let schema_sql = include_str!("../../../../../tests/fixtures/schema.sql");
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(schema_sql).unwrap();
        SqlcxIR { tables, queries: vec![], enums }
    }

    #[test]
    fn generates_zod_v4_schema() {
        let ir = parse_fixture_ir();
        let gen = ZodGenerator;
        let file = gen.generate(&ir, &HashMap::new()).unwrap();
        assert!(file.content.contains("import { z } from \"zod\""));
        assert!(file.content.contains("z.enum(["));
        assert!(file.content.contains("z.object({"));
        assert!(file.content.contains("z.infer<typeof"));
        insta::assert_snapshot!("zod_v4_schema", file.content);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core zod
```

- [ ] **Step 3: Implement ZodGenerator**

Write `zod.rs` implementing `SchemaGenerator`. Follow the same structure as `typebox.rs` but with Zod v4 API:

Type mapping:
- String → `z.string()`
- Number → `z.number()`
- Boolean → `z.boolean()`
- Date → `z.date()`
- Json → `z.unknown()` (or `@json` shape → `z.object({...})`)
- Uuid → `z.string().uuid()`
- Binary → `z.custom<Uint8Array>()`
- Enum (named) → reference to `PascalCase(enum_name)` variable
- Enum (@enum) → `z.enum(["a", "b"])`
- Unknown → `z.unknown()`
- Arrays → `z.array(inner)`
- Nullable → `.nullable()`
- Optional → `.optional()`

Generation order: imports → Prettify type → enum schemas → select/insert schemas per table → type aliases.

Import line: `import { z } from "zod";`
Enum: `export const UserStatus = z.enum(["active", "inactive", "banned"]);`
Type alias: `export type SelectUsers = Prettify<z.infer<typeof SelectUsers>>;`

Use `crate::utils::{pascal_case, escape_string}`.

- [ ] **Step 4: Add match arm in resolve_schema**

In `typescript/mod.rs`:
```rust
"zod" => Ok(Box::new(zod::ZodGenerator)),
```
And add `pub mod zod;` to the module declarations.

- [ ] **Step 5: Run test and review snapshot**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core zod && cargo insta review
```

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(rust): add Zod v4 schema generator"
```

---

### Task 20: Zod v3 Schema Generator

**Branch:** `feat/rust-20-zod-v3-generator` (from `main` after Task 17)
**Depends on:** Task 17
**Can parallel with:** Tasks 18, 19, 21
**Files:**
- Create: `sqlcx-rust/crates/sqlcx-core/src/generator/typescript/zod_v3.rs`
- Modify: `sqlcx-rust/crates/sqlcx-core/src/generator/typescript/mod.rs`

- [ ] **Step 1: Write failing snapshot test**

Same structure as Task 19 but for `ZodV3Generator`. Key difference: Binary maps to `z.instanceof(Uint8Array)` instead of `z.custom<Uint8Array>()`.

- [ ] **Step 2: Implement ZodV3Generator**

Copy `zod.rs` and change:
- Binary mapping: `z.instanceof(Uint8Array)` instead of `z.custom<Uint8Array>()`
- Everything else is identical for now

- [ ] **Step 3: Add match arm in resolve_schema**

```rust
"zod/v3" => Ok(Box::new(zod_v3::ZodV3Generator)),
```
And add `pub mod zod_v3;`.

- [ ] **Step 4: Run test and review snapshot**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core zod_v3 && cargo insta review
```

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(rust): add Zod v3 schema generator"
```

---

### Task 21: pg Driver Generator

**Branch:** `feat/rust-21-pg-driver` (from `main` after Task 17)
**Depends on:** Task 17
**Can parallel with:** Tasks 18, 19, 20
**Files:**
- Create: `sqlcx-rust/crates/sqlcx-core/src/generator/typescript/pg.rs`
- Modify: `sqlcx-rust/crates/sqlcx-core/src/generator/typescript/mod.rs`

- [ ] **Step 1: Write failing snapshot tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;
    use crate::parser::postgres::PostgresParser;
    use crate::parser::DatabaseParser;

    fn parse_fixture_ir() -> SqlcxIR {
        let schema_sql = include_str!("../../../../../tests/fixtures/schema.sql");
        let queries_sql = include_str!("../../../../../tests/fixtures/queries/users.sql");
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(schema_sql).unwrap();
        let queries = parser.parse_queries(queries_sql, &tables, &enums, "queries/users.sql").unwrap();
        SqlcxIR { tables, queries, enums }
    }

    #[test]
    fn generates_pg_client() {
        let gen = PgGenerator;
        let content = gen.generate_client();
        assert!(content.contains("import { Pool"));
        assert!(content.contains("export class PgClient implements DatabaseClient"));
        assert!(content.contains("result.rows"));
        assert!(content.contains("result.rowCount"));
        insta::assert_snapshot!("pg_client", content);
    }

    #[test]
    fn generates_pg_query_functions() {
        let ir = parse_fixture_ir();
        let gen = PgGenerator;
        let files = gen.generate(&ir).unwrap();
        let query_file = files.iter().find(|f| f.path.ends_with(".queries.ts")).unwrap();
        assert!(query_file.content.contains("export async function getUser"));
        insta::assert_snapshot!("pg_queries", query_file.content);
    }
}
```

- [ ] **Step 2: Implement PgGenerator**

Port from `bun_sql.rs` with these changes:

**Client adapter:**
```typescript
import { Pool, type QueryResult } from "pg";

export interface DatabaseClient { ... }  // same interface

export class PgClient implements DatabaseClient {
  private pool: Pool;
  constructor(pool: Pool) { this.pool = pool; }
  async query<T>(text: string, values?: unknown[]): Promise<T[]> {
    const result: QueryResult = await this.pool.query(text, values);
    return result.rows as T[];
  }
  async queryOne<T>(text: string, values?: unknown[]): Promise<T | null> {
    const rows = await this.query<T>(text, values);
    return rows[0] ?? null;
  }
  async execute(text: string, values?: unknown[]): Promise<{ rowsAffected: number }> {
    const result: QueryResult = await this.pool.query(text, values);
    return { rowsAffected: result.rowCount ?? 0 };
  }
}
```

**Query functions:** Same pattern as BunSql — `DatabaseClient` interface is identical, so query functions use the same `client.queryOne/query/execute` calls. Implement `DriverGenerator` trait with the same query-grouping logic as BunSqlGenerator.

- [ ] **Step 3: Add match arm in resolve_driver**

```rust
"pg" => Ok(Box::new(pg::PgGenerator)),
```
And add `pub mod pg;`.

- [ ] **Step 4: Run test and review snapshot**

```bash
cd sqlcx-rust && cargo test -p sqlcx-core pg && cargo insta review
```

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(rust): add pg (node-postgres) driver generator"
```

---

### Task 22: TypeScript Type-Check Verification

**Branch:** `feat/rust-22-typecheck-infra` (from `main` after Tasks 19-21)
**Depends on:** Tasks 19, 20, 21 (needs all generators to exist; does NOT depend on Task 18/MySQL)
**Files:**
- Create: `sqlcx-rust/tests/typecheck/package.json`
- Create: `sqlcx-rust/tests/typecheck/tsconfig.json`
- Create: `sqlcx-rust/tests/typecheck/.gitignore`
- Modify: `sqlcx-rust/crates/sqlcx-core/tests/e2e.rs` (or create new test)

- [ ] **Step 1: Create typecheck infrastructure**

Write `sqlcx-rust/tests/typecheck/package.json`:
```json
{
  "private": true,
  "devDependencies": {
    "@sinclair/typebox": "^0.34",
    "zod": "^4",
    "pg": "^8",
    "@types/pg": "^8",
    "typescript": "^5"
  }
}
```

Write `sqlcx-rust/tests/typecheck-zod3/package.json` (separate install for Zod v3):
```json
{
  "private": true,
  "devDependencies": {
    "zod": "^3",
    "typescript": "^5"
  }
}
```

Write `sqlcx-rust/tests/typecheck/tsconfig.json`:
```json
{
  "compilerOptions": {
    "strict": true,
    "noEmit": true,
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "skipLibCheck": true
  },
  "include": ["generated/**/*.ts"]
}
```

Copy the same `tsconfig.json` to `tests/typecheck-zod3/tsconfig.json`.

Write `sqlcx-rust/tests/typecheck/.gitignore` and `tests/typecheck-zod3/.gitignore`:
```
node_modules/
generated/
```

**Why separate directories:** Zod v3 and v4 both export as `"zod"`, so generated code always uses `import { z } from "zod"`. To type-check v3 output, we need a directory where `zod` resolves to v3. A separate `typecheck-zod3/` directory with its own `package.json` and `node_modules` achieves this cleanly.

- [ ] **Step 2: Install npm dependencies**

```bash
cd sqlcx-rust/tests/typecheck && npm install
```

- [ ] **Step 3: Write typecheck integration tests**

Add a new file `sqlcx-rust/crates/sqlcx-core/tests/typecheck.rs`:

```rust
use sqlcx_core::{
    ir::SqlcxIR,
    parser::postgres::PostgresParser,
    parser::DatabaseParser,
    generator::resolve_language,
    config::TargetConfig,
};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::fs;

fn build_fixture_ir() -> SqlcxIR {
    let schema_sql = include_str!("../../../tests/fixtures/schema.sql");
    let queries_sql = include_str!("../../../tests/fixtures/queries/users.sql");
    let parser = PostgresParser::new();
    let (tables, enums) = parser.parse_schema(schema_sql).unwrap();
    let queries = parser.parse_queries(queries_sql, &tables, &enums, "queries/users.sql").unwrap();
    SqlcxIR { tables, queries, enums }
}

fn target_config(schema: &str, driver: &str) -> TargetConfig {
    TargetConfig {
        language: "typescript".to_string(),
        out: "./generated".to_string(),
        schema: schema.to_string(),
        driver: driver.to_string(),
        overrides: HashMap::new(),
    }
}

/// Run tsc in the given typecheck directory against files in subdir.
/// Returns Ok(true) if passed, Ok(false) if npx not available, Err on tsc failure.
fn run_tsc(typecheck_base: &str, subdir: &str, files: &[sqlcx_core::generator::GeneratedFile]) -> Result<bool, String> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let gen_dir = workspace_root.join(typecheck_base).join("generated").join(subdir);
    fs::create_dir_all(&gen_dir).unwrap();

    for file in files {
        let filename = Path::new(&file.path).file_name().unwrap();
        fs::write(gen_dir.join(filename), &file.content).unwrap();
    }

    let output = Command::new("npx")
        .args(["tsc", "--noEmit", "--project", &format!("{}/tsconfig.json", typecheck_base)])
        .current_dir(&workspace_root)
        .output();

    // Clean up generated files
    fs::remove_dir_all(&gen_dir).ok();

    match output {
        Ok(result) => {
            if result.status.success() {
                Ok(true)
            } else {
                Err(format!("tsc failed:\n{}", String::from_utf8_lossy(&result.stderr)))
            }
        }
        Err(_) => Ok(false), // npx not available, skip
    }
}

#[test]
fn typebox_bunsql_typechecks() {
    let ir = build_fixture_ir();
    let plugin = resolve_language("typescript", "typebox", "bun-sql").unwrap();
    let files = plugin.generate(&ir, &target_config("typebox", "bun-sql")).unwrap();
    match run_tsc("tests/typecheck", "typebox-bunsql", &files) {
        Ok(false) => eprintln!("Skipping typecheck — npx not available"),
        Ok(true) => {},
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn zod_v4_bunsql_typechecks() {
    let ir = build_fixture_ir();
    let plugin = resolve_language("typescript", "zod", "bun-sql").unwrap();
    let files = plugin.generate(&ir, &target_config("zod", "bun-sql")).unwrap();
    match run_tsc("tests/typecheck", "zod-bunsql", &files) {
        Ok(false) => eprintln!("Skipping typecheck — npx not available"),
        Ok(true) => {},
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn zod_v3_bunsql_typechecks() {
    let ir = build_fixture_ir();
    let plugin = resolve_language("typescript", "zod/v3", "bun-sql").unwrap();
    let files = plugin.generate(&ir, &target_config("zod/v3", "bun-sql")).unwrap();
    // Zod v3 uses separate typecheck directory with zod@3
    match run_tsc("tests/typecheck-zod3", "zod3-bunsql", &files) {
        Ok(false) => eprintln!("Skipping typecheck — npx not available"),
        Ok(true) => {},
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn typebox_pg_typechecks() {
    let ir = build_fixture_ir();
    let plugin = resolve_language("typescript", "typebox", "pg").unwrap();
    let files = plugin.generate(&ir, &target_config("typebox", "pg")).unwrap();
    match run_tsc("tests/typecheck", "typebox-pg", &files) {
        Ok(false) => eprintln!("Skipping typecheck — npx not available"),
        Ok(true) => {},
        Err(e) => panic!("{}", e),
    }
}
```

Each test uses a unique subdirectory (`typebox-bunsql`, `zod-bunsql`, etc.) to avoid parallel test race conditions. Zod v3 tests run against `tests/typecheck-zod3/` which has `zod@3` installed.

- [ ] **Step 4: Run typecheck tests**

```bash
cd sqlcx-rust && cargo test typecheck
```
Expected: PASS if npm is installed, SKIP if not.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(rust): add TypeScript type-check verification infrastructure"
```

---

## Summary

| Task | Component | Depends on | Parallelizable |
|------|-----------|------------|----------------|
| 16 | Shared utils extraction | — | No |
| 17 | Trait-based dispatch refactor | 16 | No |
| 18 | MySQL parser | 17 | Yes (with 19, 20, 21) |
| 19 | Zod v4 generator | 17 | Yes (with 18, 20, 21) |
| 20 | Zod v3 generator | 17 | Yes (with 18, 19, 21) |
| 21 | pg driver generator | 17 | Yes (with 18, 19, 20) |
| 22 | TypeScript typecheck infra | 18-21 | No |

**Critical path:** 16 → 17 → (18 \|\| 19 \|\| 20 \|\| 21) → 22
**Maximum parallelism:** 4 tasks at once (18-21)
