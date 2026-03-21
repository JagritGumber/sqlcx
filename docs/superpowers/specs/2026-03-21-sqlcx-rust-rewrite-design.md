# sqlcx Rust Rewrite — Design Spec

**Date:** 2026-03-21
**Status:** Approved
**Extends:** `2026-03-18-sqlcx-design.md` (IR, annotations, output examples, core principles carry over unchanged)

## Motivation

The TypeScript prototype (sqlcx-orm v0.1.2) requires the Bun runtime — npm/Node.js users can't run it. A Rust binary solves this:

- Works everywhere via native package managers (npm, pip, cargo, brew)
- 10-100x faster parsing for large schemas
- Single binary, no runtime dependencies
- Python/Go codegen targets become plugins in the same binary

## Architecture

Cargo workspace with two crates:

```
sqlcx-rust/
├── Cargo.toml                       # Workspace root
├── crates/
│   ├── sqlcx-core/                  # Library crate (publishable to crates.io)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs               # Public API surface
│   │       ├── ir.rs                # IR structs (SqlcxIR, TableDef, QueryDef, etc.)
│   │       ├── config.rs            # Config deserialization (TOML + JSON)
│   │       ├── cache.rs             # SHA-256 IR caching
│   │       ├── annotations.rs       # @enum/@json/@param pre-processor
│   │       ├── parser/
│   │       │   ├── mod.rs           # DatabaseParser trait
│   │       │   └── postgres.rs      # PostgreSQL impl via sqlparser-rs
│   │       └── generator/
│   │           ├── mod.rs           # LanguagePlugin, SchemaGenerator, DriverGenerator traits
│   │           └── typescript/
│   │               ├── mod.rs       # TypeScript plugin orchestrator
│   │               ├── typebox.rs   # TypeBox schema generator
│   │               └── bun_sql.rs   # Bun.sql driver generator
│   └── sqlcx/                       # Binary crate (CLI)
│       ├── Cargo.toml
│       └── src/
│           └── main.rs              # CLI entry (clap), commands: generate, check, init, schema
├── tests/                           # Integration tests
│   ├── fixtures/                    # Copied from TS version
│   │   ├── schema.sql
│   │   └── queries/users.sql
│   └── e2e/
├── npm/                             # npm packaging
│   ├── sqlcx/                       # Main package (JS shim)
│   │   └── package.json
│   ├── darwin-arm64/
│   ├── darwin-x64/
│   ├── linux-x64-gnu/
│   ├── linux-arm64-gnu/
│   └── win32-x64-msvc/
└── schema/
    └── sqlcx-config.schema.json     # JSON Schema (generated from Rust structs)
```

**Key decisions:**
- Rust code lives alongside existing TS code (same repo, no separate repo)
- `sqlcx-core` is a library crate with `pub` API — publishable to crates.io
- `sqlcx` binary crate is just CLI glue, depends on `sqlcx-core`
- Test fixtures reused from the TS version verbatim

## IR (Intermediate Representation)

Direct port from the TS spec as Rust structs. All structs derive `Serialize`, `Deserialize`, `Clone`, `Debug`. The serialized JSON output is identical to the TS version.

```rust
pub struct SqlcxIR {
    pub tables: Vec<TableDef>,
    pub queries: Vec<QueryDef>,
    pub enums: Vec<EnumDef>,
}

pub struct TableDef {
    pub name: String,
    pub columns: Vec<ColumnDef>,
    pub primary_key: Vec<String>,
    pub unique_constraints: Vec<Vec<String>>,
}

pub struct ColumnDef {
    pub name: String,
    pub alias: Option<String>,
    pub source_table: Option<String>,
    pub sql_type: SqlType,
    pub nullable: bool,
    pub has_default: bool,
}

pub struct SqlType {
    pub raw: String,
    pub normalized: String,
    pub category: SqlTypeCategory,
    pub element_type: Option<Box<SqlType>>,
    pub enum_name: Option<String>,
    pub enum_values: Option<Vec<String>>,
    pub json_shape: Option<JsonShape>,
}

// Serde renames match the TS string union values for identical JSON output
#[serde(rename_all = "lowercase")]
pub enum SqlTypeCategory {
    String, Number, Boolean, Date, Json, Uuid,
    #[serde(rename = "binary")]
    Binary,
    Enum, Unknown,
    // Note: no Array variant — arrays are indicated by element_type being Some(...)
    // matching the TS behavior where category is the element's category
}

pub struct QueryDef {
    pub name: String,
    pub command: QueryCommand,
    pub sql: String,
    pub params: Vec<ParamDef>,
    pub returns: Vec<ColumnDef>,
    pub source_file: String,
}

pub enum QueryCommand { One, Many, Exec, ExecResult }

pub struct ParamDef {
    pub index: u32,
    pub name: String,
    pub sql_type: SqlType,
}

pub struct EnumDef {
    pub name: String,
    pub values: Vec<String>,
}

// Tagged union with "kind" discriminator to match TS JsonShape serialization
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum JsonShape {
    Object { fields: HashMap<String, JsonShape> },
    Array { element: Box<JsonShape> },
    String,
    Number,
    Boolean,
    Nullable { inner: Box<JsonShape> },
}

pub type Overrides = HashMap<String, String>;
```

**Memory design:**
- `Box<SqlType>` for recursive types (arrays) keeps struct sizes small
- `SqlTypeCategory` enum replaces TS string unions — exhaustive match, no stringly-typed bugs
- Generators take `&SqlcxIR` references — zero copies

## Trait System (Plugin Architecture)

Three trait axes. All plugins are compiled into a single binary. Users select plugins via config strings.

```rust
// parser/mod.rs
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

// generator/mod.rs
pub trait SchemaGenerator {
    fn generate(&self, ir: &SqlcxIR, overrides: &Overrides) -> Result<GeneratedFile>;
}

pub trait DriverGenerator {
    fn generate(&self, ir: &SqlcxIR) -> Result<GeneratedFile>;
}

pub trait LanguagePlugin {
    fn generate(&self, ir: &SqlcxIR, config: &TargetConfig) -> Result<Vec<GeneratedFile>>;
}

pub struct GeneratedFile {
    pub path: String,
    pub content: String,
}
```

**Plugin registry** — simple match functions mapping config strings to trait implementations:

```rust
pub fn resolve_parser(name: &str) -> Result<Box<dyn DatabaseParser>> {
    match name {
        "postgres" => Ok(Box::new(PostgresParser::new())),
        _ => Err(Error::UnknownParser(name.to_string())),
    }
}

pub fn resolve_language(name: &str) -> Result<Box<dyn LanguagePlugin>> {
    match name {
        "typescript" => Ok(Box::new(TypeScriptPlugin::new())),
        _ => Err(Error::UnknownLanguage(name.to_string())),
    }
}
```

Adding a new plugin = implement the trait + add a match arm. No architecture changes needed.

## PostgreSQL Parser

Uses `sqlparser-rs` crate (pure Rust, supports multiple SQL dialects). Fallback plan: hand-rolled parser if `sqlparser-rs` fights us on annotation handling or dialect quirks.

**Pipeline:**

```
SQL file content
    │
    ├─ Annotation pre-processor (annotations.rs)
    │   → Extracts @enum, @json, @param from SQL comments
    │   → Strips them before passing to sqlparser-rs
    │
    ├─ sqlparser-rs (PostgreSQL dialect)
    │   → AST for CREATE TABLE, CREATE TYPE, SELECT/INSERT/UPDATE/DELETE
    │
    └─ Post-processing
        → Resolve column types against known tables/enums
        → Infer parameter names from WHERE/SET/VALUES context
        → Apply @param overrides
        → Build IR structs
```

**Annotation pre-processing:**

```rust
pub struct Annotations {
    pub enums: HashMap<String, Vec<String>>,
    pub json_shapes: HashMap<String, JsonShape>,
    pub param_overrides: HashMap<u32, String>,
    pub query_header: Option<QueryHeader>,
}

pub struct QueryHeader {
    pub name: String,
    pub command: QueryCommand,
}

pub fn extract_annotations(sql: &str) -> (String, Annotations) { ... }
```

**Parameter name inference** — same rules as TS version:
- `WHERE id = $1` → param name: `id`
- `WHERE created_at > $1 AND created_at < $2` → `created_at_1`, `created_at_2`
- `VALUES ($1, $2, $3)` → infer from INSERT column list
- `@param $1 start_date` → explicit override wins
- Fallback: `param_1`, `param_2`, etc.

**Type category mapping** — exhaustive match for PostgreSQL types:
- `TEXT`, `VARCHAR`, `CHAR` → String
- `INT`, `SERIAL`, `BIGINT`, `SMALLINT` → Number
- `BOOLEAN` → Boolean
- `TIMESTAMP`, `DATE`, `TIME` → Date
- `JSON`, `JSONB` → Json
- `UUID` → Uuid
- `BYTEA` → ByteArray
- `TEXT[]`, `INT[]` → Array with element_type
- Named types → look up in parsed enums → Enum

## TypeScript Code Generation

Three generated files, same output as the TS version. Generators are pure functions: IR in, strings out via `write!`/`writeln!` macros (no template engine dependency).

### `schema.ts` (TypeBox schemas + type aliases)

For each table:
- `SelectTableName` schema — all columns
- `InsertTableName` schema — columns with `has_default=true` become `Optional`
- Type aliases with `Prettify<Static<typeof ...>>`

Type mapping (SqlTypeCategory → TypeBox):
| Category | TypeBox output |
|----------|---------------|
| String | `Type.String()` |
| Number | `Type.Number()` |
| Boolean | `Type.Boolean()` |
| Date | `Type.Date()` |
| Json | `Type.Any()` (or `@json` shape if annotated) |
| Uuid | `Type.String()` |
| Array | `Type.Array(elementType)` |
| Enum | `Type.Union([Type.Literal("a"), ...])` |

Nullable: `Type.Union([inner, Type.Null()])`
Optional: `Type.Optional(inner)`

### `client.ts` (DatabaseClient interface + Bun.sql adapter)

- `DatabaseClient` interface: `query<T>`, `queryOne<T>`, `execute`
- `BunSqlClient` class implementing it via Bun's `unsafe()` API

### `{queryFile}.queries.ts` (typed query functions)

For each query, grouped by source file:
- `ParamsInterface` from `query.params`
- `RowInterface` from `query.returns`
- Async function: `(client, params) → client.queryOne/query/execute`

Command mapping:
- `:one` → `queryOne<Row>(...)` → `Row | null`
- `:many` → `query<Row>(...)` → `Row[]`
- `:exec` → `execute(...)` → void
- `:execresult` → `execute(...)` → `{ rowsAffected: number }`

Overrides are applied during type mapping per config.

## Config

Dual format — TOML primary, JSON with JSON Schema. Both deserialize into the same Rust struct.

```rust
#[derive(Deserialize)]
pub struct SqlcxConfig {
    pub sql: String,
    pub parser: String,
    pub targets: Vec<TargetConfig>,
    #[serde(default)]
    pub overrides: HashMap<String, String>,
}

#[derive(Deserialize)]
pub struct TargetConfig {
    pub language: String,
    pub out: String,
    pub schema: String,
    pub driver: String,
}
```

**TOML example (`sqlcx.toml`):** (no nesting — top-level keys)
```toml
sql = "./sql"
parser = "postgres"

[[targets]]
language = "typescript"
out = "./src/db"
schema = "typebox"
driver = "bun-sql"

[overrides]
uuid = "string"
jsonb = "Record<string, unknown>"
```

**JSON example (`sqlcx.json`):**
```json
{
  "$schema": "https://unpkg.com/sqlcx-orm/schema/sqlcx-config.schema.json",
  "sql": "./sql",
  "parser": "postgres",
  "targets": [
    {
      "language": "typescript",
      "out": "./src/db",
      "schema": "typebox",
      "driver": "bun-sql"
    }
  ],
  "overrides": {
    "uuid": "string"
  }
}
```

Auto-detection: CLI looks for `sqlcx.toml` first, then `sqlcx.json`.
JSON Schema generated from Rust structs via `schemars` crate.

## Cache

Identical strategy to TS version:
- SHA-256 hash of all SQL files (sorted by path, concatenated)
- Stored at `.sqlcx/ir.json` with hash field
- Cache hit: hash matches → skip parsing, load IR from JSON
- Cache miss: re-parse, write new IR + hash atomically (temp file → rename)
- Adding a new language target reuses cached IR — no re-parse

Crates: `sha2`, `serde_json`, `tempfile`.

## CLI

Via `clap` crate with derive macros:

```
sqlcx generate    # Parse SQL → IR → generate code for all targets
sqlcx check       # Validate SQL + config without writing files
sqlcx init        # Scaffold config + sql/ directory + example files
sqlcx schema      # Emit JSON Schema for config validation to stdout
```

## Error Handling

Structured errors via `thiserror`, no panics anywhere:

```rust
#[derive(thiserror::Error, Debug)]
pub enum SqlcxError {
    #[error("Config file not found: {0}")]
    ConfigNotFound(String),
    #[error("Invalid config: {0}")]
    ConfigInvalid(String),
    #[error("SQL parse error in {file}: {message}")]
    ParseError { file: String, message: String },
    #[error("Unknown column type: {0}")]
    UnknownType(String),
    #[error("Missing query annotation in {file}")]
    MissingAnnotation { file: String },
    #[error("Unknown parser: {0}")]
    UnknownParser(String),
    #[error("Unknown language: {0}")]
    UnknownLanguage(String),
    #[error("Unknown schema generator: {0}")]
    UnknownSchema(String),
    #[error("Unknown driver generator: {0}")]
    UnknownDriver(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
```

All public functions return `Result<T, SqlcxError>`. CLI catches at top level, prints human-readable messages. Exit code 1 on any error.

## Distribution

Native package manager support across all ecosystems:

| Channel | Package name | Mechanism |
|---------|-------------|-----------|
| npm | `sqlcx-orm` + `@sqlcx/*` platform pkgs | Optional deps, JS shim |
| PyPI | `sqlcx` | Python wheel with embedded binary (ruff/uv model) |
| crates.io | `sqlcx` | `cargo install` builds from source |
| Homebrew | `sqlcx` | Formula pointing to GitHub Release tarball |
| GitHub Releases | `sqlcx-*` binaries | Standalone downloads with checksums |

**npm platform packages:**
- `@sqlcx/darwin-arm64` (macOS Apple Silicon)
- `@sqlcx/darwin-x64` (macOS Intel)
- `@sqlcx/linux-x64-gnu` (Linux x64)
- `@sqlcx/linux-arm64-gnu` (Linux ARM64)
- `@sqlcx/win32-x64-msvc` (Windows x64)

**Cross-compilation targets:**
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-pc-windows-msvc`

CI builds all targets, strips binaries, and publishes to all package managers atomically.

## Testing Strategy

| Layer | Test type | What's tested |
|-------|-----------|---------------|
| IR | Unit | Serialization round-trip (struct → JSON → struct) |
| Annotations | Unit | `@enum`, `@json`, `@param` extraction from SQL comments |
| Parser | Unit | Schema parsing, query parsing, param inference, type mapping |
| Generators | Snapshot | Generated TypeScript output matches expected fixtures |
| Cache | Unit | Hash computation, cache hit/miss, atomic writes |
| Config | Unit | TOML and JSON deserialization, validation |
| CLI | Integration | Full pipeline: SQL files → generated output directory |
| E2E | Snapshot | Fixture SQL → complete generated project matches expected output |

Snapshot testing via `insta` crate. Test fixtures reused from TS version. The Rust version must generate identical TypeScript output to the TS version.

## Crate Dependencies

| Crate | Purpose | Size impact |
|-------|---------|-------------|
| `clap` (derive) | CLI argument parsing | ~200KB |
| `serde` + `serde_json` | IR serialization, config | ~150KB |
| `toml` | TOML config parsing | ~50KB |
| `sqlparser` | SQL AST parsing | ~300KB |
| `sha2` | Content hashing for cache | ~30KB |
| `thiserror` | Structured error types | ~5KB |
| `glob` | SQL file discovery | ~10KB |
| `schemars` | JSON Schema generation | ~100KB (build-time) |
| `tempfile` | Atomic file writes | ~10KB |
| `insta` | Snapshot testing | dev-only |

Estimated binary size: ~2-4MB stripped.

## v1 Scope

**In scope:**
- CLI: `init`, `generate`, `check`, `schema`
- IR: complete, serializable, JSON-cacheable (identical to TS version)
- Parser: PostgreSQL (via `sqlparser-rs`)
- Language: TypeScript
- Schema: TypeBox v1.0
- Driver: Bun.sql
- Annotations: `@enum`, `@json`, `@param` (sqlc-compatible)
- Config: TOML + JSON with JSON Schema
- Distribution: npm, PyPI, crates.io, Homebrew, GitHub Releases
- Caching: SHA-256 content hash

**v1 limitations (supported with caveats):**
- JOIN queries: supported with explicit column aliases required (no bare `SELECT *` on JOINs)
- `SELECT *` on single tables: fully supported (expanded via schema)
- `schema` CLI command is new (not in TS version) — emits JSON Schema for config validation

**Out of scope (designed for, not built in v1):**
- Python/Go/Rust codegen
- MySQL/SQLite parsers
- Zod/Valibot/Pydantic schema generators
- pg/mysql2/asyncpg driver generators
- Watch mode
- Migration generation
- DSL compiler (`.sqlcx` files — see `2026-03-18-sqlcx-dsl-design.md`)
