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

## 1. Shared Utilities Extraction

Extract duplicated helpers from `typebox.rs` and `bun_sql.rs` into a new `utils.rs`:

```rust
// crates/sqlcx-core/src/utils.rs
pub fn pascal_case(s: &str) -> String       // snake_case → PascalCase
pub fn camel_case(s: &str) -> String        // snake_case → camelCase
pub fn split_words(s: &str) -> String       // PascalCase → snake_case splitter
pub fn escape_string(s: &str) -> String     // JSON.stringify-style escaping for TS strings
```

Existing generators updated to `use crate::utils::*`. No behavior change — just dedup.

## 2. MySQL Parser

New file: `crates/sqlcx-core/src/parser/mysql.rs`

Implements `DatabaseParser` trait for MySQL 8.0+. Same regex-based approach as the Postgres parser.

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

Nearly identical to v4 for our use case. Uses `z.object()` and `z.infer` which work in both versions. The split exists so we can diverge as v4 evolves.

### Type mapping (SqlTypeCategory → Zod)

| Category | Zod output |
|----------|-----------|
| String | `z.string()` |
| Number | `z.number()` |
| Boolean | `z.boolean()` |
| Date | `z.date()` |
| Json | `z.unknown()` (or `@json` shape → `z.object({...})`) |
| Uuid | `z.string().uuid()` |
| Binary | `z.instanceof(Uint8Array)` |
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

Schema resolution happens inside `TypeScriptPlugin`. The plugin needs to resolve schema by name:

```rust
fn resolve_schema(name: &str) -> Result<Box<dyn SchemaGenerator>> {
    match name {
        "typebox" => Ok(Box::new(TypeBoxGenerator)),
        "zod" => Ok(Box::new(ZodGenerator)),
        "zod/v3" => Ok(Box::new(ZodV3Generator)),
        _ => Err(SqlcxError::UnknownSchema(name.to_string())),
    }
}
```

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

Query functions are identical in structure to Bun.sql — same interfaces, same SQL constants, same async functions. Only the client adapter differs.

### Resolver update

Driver resolution happens inside `TypeScriptPlugin`:

```rust
fn resolve_driver(name: &str) -> Result<Box<dyn DriverGenerator>> {
    match name {
        "bun-sql" => Ok(Box::new(BunSqlGenerator)),
        "pg" => Ok(Box::new(PgGenerator)),
        _ => Err(SqlcxError::UnknownDriver(name.to_string())),
    }
}
```

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
    "pg": "^8",
    "@types/pg": "^8",
    "typescript": "^5.9"
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

If npm/bun is not available, the test is skipped (not failed). This catches:
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
