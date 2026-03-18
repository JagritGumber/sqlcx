# sqlcx — SQL-First Cross-Language Type-Safe Code Generator

**Date:** 2026-03-18
**Status:** Approved

## Problem

ORMs couple your data layer to a single language runtime. Prisma tried to be universal but died by coupling to TypeScript. Drizzle has type gaps (e.g., `$type<>()` inference breaks with TypeBox). sqlc proved that SQL-first codegen works beautifully for Go, but it's single-language.

There is no tool that:
- Treats SQL as the source of truth
- Generates type-safe code for multiple languages from one set of SQL files
- Supports open validation schemas (TypeBox, Zod, Pydantic, etc.)
- Stays lightweight — no runtime, just codegen

## Solution

**sqlcx** is a build-time code generator. You write SQL schemas and annotated queries, sqlcx parses them into a language-agnostic Intermediate Representation (IR), then language plugins generate idiomatic, type-safe code for each target.

```
SQL files → [DatabaseParser] → IR (cached, serializable)
                                    │
                          ┌─────────┼──────────┐
                          ▼         ▼          ▼
                    [TypeScript]  [Python]   [Go]
                      │              │          │
                  ┌───┴───┐     ┌───┴───┐     ...
                  ▼       ▼     ▼       ▼
              [TypeBox] [Bun]  [Pydantic] [asyncpg]
```

## Core Principles

1. **SQL is the source of truth** — no schema definitions in TypeScript/Python/Go
2. **Build-time only** — zero runtime dependency on sqlcx
3. **IR is the product** — a clean, serializable representation that any language plugin consumes
4. **Open validation API** — Standard Schema (`@standard-schema/spec`) compatible; TypeBox v1.0 default
5. **Input/Output type separation** — inspired by better-auth; INSERT types differ from SELECT types
6. **Prettify for DX** — all generated types wrapped so IDE hover shows expanded shapes

## Query Annotation Format

sqlc-compatible comment annotations:

```sql
-- name: GetUser :one
SELECT * FROM users WHERE id = $1;

-- name: ListUsers :many
SELECT id, name, email FROM users WHERE name ILIKE $1;

-- name: CreateUser :exec
INSERT INTO users (name, email, bio) VALUES ($1, $2, $3);

-- name: DeleteUser :execresult
DELETE FROM users WHERE id = $1;
```

Commands:
- `:one` — returns single row or null
- `:many` — returns array of rows
- `:exec` — returns nothing
- `:execresult` — returns affected rows count

## Intermediate Representation (IR)

The IR is the language-agnostic bridge. It is JSON-serializable for caching and tooling.

```ts
interface SqlcxIR {
  tables: TableDef[];
  queries: QueryDef[];
  enums: EnumDef[];
}

interface TableDef {
  name: string;
  columns: ColumnDef[];
  primaryKey: string[];
  uniqueConstraints: string[][];
}

interface ColumnDef {
  name: string;
  alias?: string;         // e.g. "profile_id" for `p.id AS profile_id`
  sourceTable?: string;   // originating table for JOIN queries
  type: SqlType;          // normalized SQL type
  nullable: boolean;
  hasDefault: boolean;    // affects Insert vs Select type generation
}

interface SqlType {
  raw: string;           // original SQL type string e.g. "VARCHAR(255)"
  normalized: string;    // normalized key e.g. "varchar"
  category: "string" | "number" | "boolean" | "date" | "json" | "uuid" | "binary" | "enum" | "unknown";
  elementType?: SqlType; // for array types: TEXT[] → category "string", elementType: { category: "string" }
  enumName?: string;     // for enum types: references EnumDef.name
}

interface QueryDef {
  name: string;          // from annotation
  command: "one" | "many" | "exec" | "execresult";
  sql: string;           // original SQL
  params: ParamDef[];    // ordered parameters
  returns: ColumnDef[];  // inferred return columns (empty for :exec)
  sourceFile: string;
}

interface ParamDef {
  index: number;         // $1, $2, etc.
  name: string;          // inferred from column name in WHERE/SET clause
  type: SqlType;
}

// Parameter name inference rules:
// 1. Simple: `WHERE id = $1` → name: "id"
// 2. Collision: `WHERE created_at > $1 AND created_at < $2` → "created_at_1", "created_at_2"
// 3. Expression: `WHERE LOWER(name) = $1` → "name" (extract column from expression)
// 4. Unresolvable: fallback to "param_1", "param_2", etc.
// 5. Annotation override: `-- @param $1 start_date` takes precedence over inference

interface EnumDef {
  name: string;
  values: string[];
}
```

### IR Caching

The IR is cached to disk at `.sqlcx/ir.json`. Cache invalidation uses a content hash
of all SQL input files (SHA-256 of concatenated file contents, sorted by path).
If the hash matches, the cached IR is reused. If any SQL file changes, the IR is
regenerated. `sqlcx check` validates both the SQL files and the cached IR freshness.
Adding a new language target does not re-parse SQL — it reads the cached IR.

## Plugin Architecture

Four plugin axes:

### 1. Database Parser

Parses SQL dialect into the IR.

```ts
interface DatabaseParser {
  dialect: string;
  parseSchema(sql: string): TableDef[];
  parseQueries(sql: string, tables: TableDef[]): QueryDef[];
  parseEnums(sql: string): EnumDef[];
}
```

v1 ships: PostgreSQL parser (via `node-sql-parser`).
Future: MySQL, SQLite, custom parsers, `libpg_query-wasm` for exact Postgres fidelity.

### 2. Language Plugin

Each language target bundles its own schema generator and driver generator.

```ts
interface LanguageOptions {
  out: string;              // output directory
  overrides?: Record<string, string>;  // SQL type → language type overrides
}

interface LanguagePlugin {
  language: string;
  fileExtension: string;
  generate(ir: SqlcxIR, options: LanguageOptions): GeneratedFile[];
}

interface GeneratedFile {
  path: string;
  content: string;
}

// Overrides flow: config.overrides are passed through LanguageOptions to each target.
// They are applied at codegen time (NOT baked into the IR), so the IR stays
// language-agnostic and cacheable across different target configurations.
```

v1 ships: TypeScript.
Future: Python, Go, Rust, etc.

### 3. Schema Generator (per language)

Generates validation schemas + inferred types from IR types.

```ts
interface SchemaGenerator {
  name: string;
  generateImports(): string;
  generateEnumSchema(enumDef: EnumDef): string;
  generateSelectSchema(table: TableDef, ir: SqlcxIR): string;
  generateInsertSchema(table: TableDef, ir: SqlcxIR): string;
  generateTypeAlias(name: string, schemaVarName: string): string;
}
```

v1 ships: TypeBox v1.0 (outputs JSON Schema, Standard Schema compatible).
Future: Zod, Valibot, ArkType for TS; Pydantic, msgspec for Python.

### 4. Driver Generator (per language)

Generates the runtime query functions that call the database driver.

```ts
interface DriverGenerator {
  name: string;
  generateImports(): string;
  generateClientAdapter(): string;   // concrete adapter implementing DatabaseClient
  generateQueryFunction(query: QueryDef): string;
}

// DatabaseClient is a fixed, driver-agnostic interface generated by the
// LanguagePlugin (not the DriverGenerator). Each driver generates an adapter
// that implements DatabaseClient. This way multiple drivers don't collide —
// they produce separate adapter files, all implementing the same interface.
```

v1 ships: `Bun.sql` adapter.
Future: `pg`, `mysql2`, `better-sqlite3`, `asyncpg`, `pgx`.

## Configuration

```ts
// sqlcx.config.ts
import { defineConfig } from "sqlcx";
import { postgresParser } from "sqlcx/parser/postgres";
import { typescript } from "sqlcx/lang/typescript";
import { typebox } from "sqlcx/schema/typebox";
import { bunSql } from "sqlcx/driver/bun-sql";

export default defineConfig({
  sql: "./sql",
  parser: postgresParser(),
  targets: [
    typescript({
      out: "./src/db",
      schema: typebox(),
      driver: bunSql(),
    }),
    // Future:
    // python({ out: "./python/db", schema: pydantic(), driver: asyncpg() }),
  ],
  overrides: {
    "uuid": "string",
    "jsonb": "Record<string, unknown>",
  },
});
```

## Generated Output Example

Given:
```sql
CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  email TEXT NOT NULL UNIQUE,
  bio TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
```

### `src/db/schema.ts`:
```ts
import { Type, Static } from "typebox";

export const SelectUser = Type.Object({
  id: Type.Number(),
  name: Type.String(),
  email: Type.String(),
  bio: Type.Union([Type.String(), Type.Null()]),
  created_at: Type.Date(),
});

export const InsertUser = Type.Object({
  name: Type.String(),
  email: Type.String(),
  bio: Type.Optional(Type.Union([Type.String(), Type.Null()])),
  // id and created_at omitted — they have defaults
});

export type SelectUser = Prettify<Static<typeof SelectUser>>;
export type InsertUser = Prettify<Static<typeof InsertUser>>;
```

### `src/db/users.queries.ts`:
```ts
import type { DatabaseClient } from "./client";
import type { SelectUser } from "./schema";

const getUserSQL = `SELECT * FROM users WHERE id = $1`;

export async function getUser(
  client: DatabaseClient,
  params: { id: number }
): Promise<SelectUser | null> {
  return client.queryOne<SelectUser>(getUserSQL, [params.id]);
}
```

### `src/db/client.ts`:
```ts
export interface DatabaseClient {
  query<T>(sql: string, params: unknown[]): Promise<T[]>;
  queryOne<T>(sql: string, params: unknown[]): Promise<T | null>;
  execute(sql: string, params: unknown[]): Promise<{ rowsAffected: number }>;
}
```

## Type Patterns (Learned from better-auth)

1. **`Prettify<T>`** — expands types in IDE hover for better DX
2. **Input/Output separation** — `InsertUser` omits columns with defaults; `SelectUser` includes all
3. **Nullable handling** — `NULL` columns become `Type.Union([Type.X(), Type.Null()])`
4. **Optional for inserts** — nullable columns without defaults are `Type.Optional()`
5. **Standard Schema compliance** — TypeBox v1.0 implements `@standard-schema/spec`

## CLI Commands

- `sqlcx init` — scaffold `sqlcx.config.ts`, `sql/` directory, example files
- `sqlcx generate` — parse SQL → IR → generate code for all targets
- `sqlcx check` — validate SQL files and config without generating (CI use)

## Project Structure

```
sqlcx/
├── src/
│   ├── cli/              # CLI entry (bun)
│   │   └── index.ts
│   ├── config/           # defineConfig + config loading
│   │   └── index.ts
│   ├── ir/               # IR types + serialization
│   │   └── index.ts
│   ├── parser/           # Parser plugin interface + implementations
│   │   ├── interface.ts
│   │   └── postgres.ts
│   ├── generator/        # Language plugin interface
│   │   ├── interface.ts
│   │   └── typescript/
│   │       ├── index.ts
│   │       ├── schema/
│   │       │   ├── interface.ts
│   │       │   └── typebox.ts
│   │       └── driver/
│   │           ├── interface.ts
│   │           └── bun-sql.ts
│   └── utils/
│       └── index.ts
├── tests/
├── package.json
├── tsconfig.json
└── bunfig.toml
```

## v1 Scope

**In scope:**
- CLI: `init`, `generate`, `check`
- IR: complete, serializable, JSON-cacheable
- Parser: PostgreSQL (via `node-sql-parser`)
- Language: TypeScript
- Schema: TypeBox v1.0
- Driver: `Bun.sql`
- Annotations: sqlc-compatible

**v1 Limitations (designed for, basic support):**
- JOIN queries: supported with explicit column aliases required (no bare `SELECT *` on JOINs)
- `SELECT *` on single tables: fully supported (expanded via schema)

**Out of scope (designed for, not built):**
- Python/Go/Rust codegen
- MySQL/SQLite parsers
- Watch mode
- Migration generation
- Custom annotation extensions

## Tech Stack

- **Runtime:** Bun
- **Language:** TypeScript
- **SQL Parsing:** `node-sql-parser`
- **Testing:** Bun's built-in test runner
- **Build:** Bun's built-in bundler
