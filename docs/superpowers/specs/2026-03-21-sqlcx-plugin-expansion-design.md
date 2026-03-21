# sqlcx Plugin Expansion — MySQL, Zod, pg

**Date:** 2026-03-21
**Status:** Approved
**Extends:** `2026-03-21-sqlcx-rust-rewrite-design.md`

## Goal

Expand sqlcx from 1x1x1x1 (Postgres/TypeScript/TypeBox/Bun.sql) to 2x1x3x2, adding:
- MySQL 8.0+ parser
- Zod v4 and Zod v3 schema generators
- pg (node-postgres) driver generator
- Shared utility extraction
- TypeScript type-check verification infrastructure

## 0. Prerequisite: Refactor TypeScriptPlugin to Use Trait Dispatch

The current `TypeScriptPlugin::generate()` hard-codes `TypeBoxGenerator` and `BunSqlGenerator` directly — it does not use the `SchemaGenerator`/`DriverGenerator` traits. Before adding new plugins, refactor to use trait-based dispatch:

1. **Make `TypeBoxGenerator` implement `SchemaGenerator` trait** (currently it has ad-hoc methods `generate_schema_file()`)
2. **Make `BunSqlGenerator` implement `DriverGenerator` trait** (currently it has ad-hoc methods `generate_client()` and `generate_query_functions()`)
3. **Add resolver functions** inside `typescript/mod.rs`:

```rust
fn resolve_schema(name: &str) -> Result<Box<dyn SchemaGenerator>> {
    match name {
        "typebox" => Ok(Box::new(TypeBoxGenerator)),
        "zod" => Ok(Box::new(ZodGenerator)),
        "zod/v3" => Ok(Box::new(ZodV3Generator)),
        _ => Err(SqlcxError::UnknownSchema(name.to_string())),
    }
}

fn resolve_driver(name: &str) -> Result<Box<dyn DriverGenerator>> {
    match name {
        "bun-sql" => Ok(Box::new(BunSqlGenerator)),
        "pg" => Ok(Box::new(PgGenerator)),
        _ => Err(SqlcxError::UnknownDriver(name.to_string())),
    }
}
```

4. **Update `TypeScriptPlugin::generate()`** to call resolvers:

```rust
impl LanguagePlugin for TypeScriptPlugin {
    fn generate(&self, ir: &SqlcxIR, config: &TargetConfig) -> Result<Vec<GeneratedFile>> {
        let schema_gen = resolve_schema(&self.schema_name)?;
        let driver_gen = resolve_driver(&self.driver_name)?;
        let overrides = config.overrides(); // wire through from config

        let mut files = Vec::new();
        files.push(schema_gen.generate(ir, &overrides)?);          // schema.ts
        files.extend(driver_gen.generate(ir)?);                     // client.ts + query files
        Ok(files)
    }
}
```

5. **Wire `overrides` from config** — currently `TypeScriptPlugin::generate()` creates `HashMap::new()` instead of reading from config. Add an `overrides()` method to `TargetConfig` or pass `config.overrides` from the CLI level through to the plugin.

**The `DriverGenerator` trait signature** is `fn generate(&self, ir: &SqlcxIR) -> Result<Vec<GeneratedFile>>` — it returns multiple files (client.ts + one .queries.ts per source file). The driver owns both client adapter generation and query function generation.

## 1. Shared Utilities Extraction

Extract duplicated helpers from `typebox.rs` and `bun_sql.rs` into a new `utils.rs`:

```rust
// crates/sqlcx-core/src/utils.rs
pub fn pascal_case(s: &str) -> String       // snake_case → PascalCase
pub fn camel_case(s: &str) -> String        // snake_case → camelCase
pub fn split_words(s: &str) -> String       // Insert underscores between camelCase/PascalCase words (e.g., "GetUser" → "get_user") — used before pascal_case/camel_case for re-casing
pub fn escape_string(s: &str) -> String     // JSON.stringify-style escaping for TS strings
```

Existing generators updated to `use crate::utils::*`. No behavior change — just dedup.

## 2. MySQL Parser

New file: `crates/sqlcx-core/src/parser/mysql.rs`

Implements `DatabaseParser` trait for MySQL 8.0+. Uses regex-based parsing (same approach as the current Postgres parser implementation, which also uses regex despite the base spec mentioning sqlparser-rs). The `sqlparser-rs` crate supports MySQL dialect and could be used as an alternative, but regex is proven and consistent with the Postgres parser.

### MySQL vs Postgres differences

| Feature | Postgres | MySQL |
|---------|----------|-------|
| Auto-increment | `SERIAL` type | `AUTO_INCREMENT` keyword |
| Enums | `CREATE TYPE name AS ENUM (...)` | Inline `ENUM('a','b','c')` on column |
| Booleans | `BOOLEAN` type | `TINYINT(1)` or `BOOLEAN` (alias) |
| JSON | `JSON`, `JSONB` | `JSON` only |
| Arrays | `TEXT[]`, `INT[]` | Not supported |
| Strings | `TEXT`, `VARCHAR` | `TEXT`, `VARCHAR`, `CHAR`, `TINYTEXT`, `MEDIUMTEXT`, `LONGTEXT` |
| Numbers | `SERIAL`, `BIGSERIAL` | `INT AUTO_INCREMENT`, `BIGINT AUTO_INCREMENT` |
| Unsigned | N/A | `INT UNSIGNED`, `BIGINT UNSIGNED` |
| Dates | `TIMESTAMP`, `TIMESTAMPTZ` | `DATETIME`, `TIMESTAMP` (no timezone) |
| Binary | `BYTEA` | `BLOB`, `TINYBLOB`, `MEDIUMBLOB`, `LONGBLOB`, `VARBINARY` |
| Default | `DEFAULT NOW()` | `DEFAULT CURRENT_TIMESTAMP` |
| Backtick quoting | Double quotes | Backticks |
| Parameters | `$1, $2` | `?` (positional) |

### MySQL type mapping

| SQL Type | Category |
|----------|----------|
| `TINYINT(1)` | Boolean |
| `TINYINT`, `SMALLINT`, `MEDIUMINT`, `INT`, `BIGINT` (+ `UNSIGNED`) | Number |
| `FLOAT`, `DOUBLE`, `DECIMAL`, `NUMERIC` | Number |
| `CHAR`, `VARCHAR`, `TINYTEXT`, `TEXT`, `MEDIUMTEXT`, `LONGTEXT` | String |
| `DATETIME`, `TIMESTAMP`, `DATE`, `TIME`, `YEAR` | Date |
| `JSON` | Json |
| `BINARY`, `VARBINARY`, `TINYBLOB`, `BLOB`, `MEDIUMBLOB`, `LONGBLOB` | Binary |
| `ENUM('a','b')` | Enum (values extracted from column definition) |

### MySQL parameter handling

MySQL uses `?` positional placeholders instead of `$1, $2`. The parser:
- Counts `?` occurrences left-to-right to determine param index
- Infers names from context (WHERE/SET/VALUES) same as Postgres
- `@param` overrides use 1-based index: `-- @param $1 start_date` maps to the first `?`

### MySQL 8.0+ features supported

- `JSON` column type
- `GENERATED ALWAYS AS` columns (treated as `has_default = true`)
- Standard CREATE TABLE syntax with backtick quoting
- Inline `ENUM('val1', 'val2')` on columns

### MySQL Test Fixtures

**`tests/fixtures/mysql_schema.sql`:**
```sql
CREATE TABLE users (
  id INT AUTO_INCREMENT PRIMARY KEY,
  name VARCHAR(255) NOT NULL,
  email VARCHAR(255) NOT NULL UNIQUE,
  bio TEXT,
  role ENUM('admin', 'user', 'guest') NOT NULL DEFAULT 'user',
  preferences JSON,
  is_active TINYINT(1) NOT NULL DEFAULT 1,
  avatar BLOB,
  score DECIMAL(10, 2) UNSIGNED,
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

**`tests/fixtures/mysql_queries/users.sql`:**
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

### Config

```toml
parser = "mysql"
```

### Resolver update

```rust
// parser/mod.rs
pub fn resolve_parser(name: &str) -> Result<Box<dyn DatabaseParser>> {
    match name {
        "postgres" => Ok(Box::new(postgres::PostgresParser::new())),
        "mysql" => Ok(Box::new(mysql::MySqlParser::new())),
        _ => Err(SqlcxError::UnknownParser(name.to_string())),
    }
}
```

## 3. Zod Schema Generators

Two generators following Zod's own naming convention:
- `schema = "zod"` → Zod v4 (current/default)
- `schema = "zod/v3"` → Zod v3 (legacy)

### Files

- `crates/sqlcx-core/src/generator/typescript/zod.rs` — Zod v4
- `crates/sqlcx-core/src/generator/typescript/zod_v3.rs` — Zod v3

Both implement `SchemaGenerator` trait.

### Zod v4 output example (`schema = "zod"`)

```typescript
import { z } from "zod";

type Prettify<T> = { [K in keyof T]: T[K] } & {};

export const UserStatus = z.enum(["active", "inactive", "banned"]);

export const SelectUsers = z.object({
  id: z.number(),
  name: z.string(),
  email: z.string(),
  bio: z.string().nullable(),
  status: UserStatus,
  tags: z.array(z.string()).optional(),
  created_at: z.date(),
});

export const InsertUsers = z.object({
  name: z.string(),
  email: z.string(),
  bio: z.string().nullable().optional(),
  status: UserStatus.optional(),
  tags: z.array(z.string()).optional(),
});

export type SelectUsers = Prettify<z.infer<typeof SelectUsers>>;
export type InsertUsers = Prettify<z.infer<typeof InsertUsers>>;
export type UserStatus = Prettify<z.infer<typeof UserStatus>>;
```

### Zod v3 output (`schema = "zod/v3"`)

The v3 generator produces identical output to v4 for our current use case — `z.object()`, `z.enum()`, `z.infer` all work the same in both versions. The key difference: v3 uses `z.instanceof(Uint8Array)` for Binary (available in v3), while v4 uses `z.custom<Uint8Array>()` (since `z.instanceof` was removed in v4). The two generators exist as separate files so we can diverge as v4's API evolves without breaking v3 users.

### Type mapping (SqlTypeCategory → Zod)

| Category | Zod output |
|----------|-----------|
| String | `z.string()` |
| Number | `z.number()` |
| Boolean | `z.boolean()` |
| Date | `z.date()` |
| Json | `z.unknown()` (or `@json` shape → `z.object({...})`) |
| Uuid | `z.string().uuid()` |
| Binary | v4: `z.custom<Uint8Array>()`, v3: `z.instanceof(Uint8Array)` |
| Enum (named) | `PascalCase(enum_name)` reference |
| Enum (@enum) | `z.enum(["a", "b"])` |
| Unknown | `z.unknown()` |

Nullable: `.nullable()`
Optional: `.optional()`

### JSON shape mapping (Zod)

| JsonShape | Zod output |
|-----------|-----------|
| String | `z.string()` |
| Number | `z.number()` |
| Boolean | `z.boolean()` |
| Object { fields } | `z.object({ key: value, ... })` |
| Array { element } | `z.array(element)` |
| Nullable { inner } | `inner.nullable()` |

### Resolver update

New match arms added to `resolve_schema()` in `typescript/mod.rs` (defined in Section 0).

## 4. pg Driver Generator

New file: `crates/sqlcx-core/src/generator/typescript/pg.rs`

### Generated client.ts (pg variant)

```typescript
import { Pool, type QueryResult } from "pg";

export interface DatabaseClient {
  query<T>(sql: string, params: unknown[]): Promise<T[]>;
  queryOne<T>(sql: string, params: unknown[]): Promise<T | null>;
  execute(sql: string, params: unknown[]): Promise<{ rowsAffected: number }>;
}

export class PgClient implements DatabaseClient {
  private pool: Pool;

  constructor(pool: Pool) {
    this.pool = pool;
  }

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

Query functions have the same interface structure as Bun.sql (same row/params interfaces, same SQL constants) but the function bodies differ — pg uses `pool.query(text, values)` while Bun.sql uses `sql.unsafe(text, values)`. Example:

```typescript
// pg version
export async function getUser(client: DatabaseClient, params: GetUserParams): Promise<GetUserRow | null> {
  return client.queryOne<GetUserRow>(getUserSql, [params.id]);
}
```

The `DatabaseClient` interface is identical across drivers — only the adapter class implementation differs. Query functions call the interface methods, not the driver directly.

### Resolver update

New match arm added to `resolve_driver()` in `typescript/mod.rs` (defined in Section 0).

### Config

```toml
driver = "pg"
```

## 5. TypeScript Type-Check Verification

New directory: `sqlcx-rust/tests/typecheck/`

```
tests/typecheck/
├── package.json          # devDeps for all supported libraries
├── tsconfig.json         # strict, noEmit
└── generated/            # output target for test-generated files (gitignored)
```

### package.json

```json
{
  "private": true,
  "devDependencies": {
    "@sinclair/typebox": "^0.34",
    "zod": "^4",
    "zod3": "npm:zod@^3",
    "pg": "^8",
    "@types/pg": "^8",
    "typescript": "^5"
  }
}
```

### tsconfig.json

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

### Integration test

For each schema+driver combo, the test:
1. Generates TypeScript files into `tests/typecheck/generated/`
2. Runs `npx tsc --noEmit --project tests/typecheck/tsconfig.json`
3. Asserts exit code 0

**Snapshot tests:** Each new generator (Zod v4, Zod v3, pg) also gets `insta` snapshot tests, same as TypeBox and Bun.sql. These verify the generated output format; the `tsc` tests verify the output actually compiles.

If npm/bun is not available, the `tsc` test is skipped (not failed). This catches:
- Wrong import paths
- Incorrect TypeBox/Zod/pg API usage
- Type mismatches in generated interfaces
- Missing or wrong generic parameters

## File Map

```
crates/sqlcx-core/src/
├── utils.rs                                    # NEW: shared helpers
├── parser/
│   ├── mod.rs                                  # MODIFIED: add mysql match arm
│   ├── postgres.rs                             # EXISTING
│   └── mysql.rs                                # NEW: MySQL parser
└── generator/
    ├── mod.rs                                  # MODIFIED: update resolve_language
    └── typescript/
        ├── mod.rs                              # MODIFIED: add schema/driver resolvers
        ├── typebox.rs                          # MODIFIED: use crate::utils
        ├── bun_sql.rs                          # MODIFIED: use crate::utils
        ├── zod.rs                              # NEW: Zod v4
        ├── zod_v3.rs                           # NEW: Zod v3
        └── pg.rs                               # NEW: pg driver

tests/
├── typecheck/                                  # NEW: tsc verification
│   ├── package.json
│   ├── tsconfig.json
│   └── generated/                              # gitignored
├── fixtures/
│   ├── schema.sql                              # EXISTING (Postgres)
│   ├── mysql_schema.sql                        # NEW: MySQL test fixture
│   ├── queries/
│   │   └── users.sql                           # EXISTING
│   └── mysql_queries/
│       └── users.sql                           # NEW: MySQL query fixture (? params)
```

## v0.2.0 Scope Summary

After this expansion:

| Axis | Before | After |
|------|--------|-------|
| Parsers | PostgreSQL | PostgreSQL, MySQL |
| Schema (TS) | TypeBox | TypeBox, Zod v4, Zod v3 |
| Drivers (TS) | Bun.sql | Bun.sql, pg |
| Type verification | Snapshot only | Snapshot + `tsc --noEmit` |
