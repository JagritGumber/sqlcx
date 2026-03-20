# sqlcx

SQL-first, cross-language, type-safe code generator. Write SQL, get typed code.

```bash
bun add sqlcx-orm
```

## What it does

You write SQL schemas and annotated queries. sqlcx generates type-safe TypeScript with [TypeBox](https://github.com/sinclairzx81/typebox) validation schemas and typed query functions.

```
sql/schema.sql          -->  src/db/schema.ts       (TypeBox schemas + types)
sql/queries/users.sql   -->  src/db/users.queries.ts (typed query functions)
                              src/db/client.ts        (DatabaseClient interface)
```

No runtime. No engine. Just codegen.

## Quick Start

```bash
# Initialize a project
bunx sqlcx-orm init

# Write your SQL, then generate
bunx sqlcx-orm generate
```

### 1. Define your schema

```sql
-- sql/schema.sql
CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  email TEXT NOT NULL UNIQUE,
  -- @json({ theme: string, notifications: boolean })
  preferences JSONB,
  -- @enum("admin", "editor", "viewer")
  role TEXT NOT NULL DEFAULT 'viewer',
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
```

### 2. Write annotated queries

```sql
-- sql/queries/users.sql

-- name: GetUser :one
SELECT * FROM users WHERE id = $1;

-- name: ListUsers :many
SELECT id, name, email, role FROM users ORDER BY created_at DESC;

-- name: CreateUser :one
INSERT INTO users (name, email, role) VALUES ($1, $2, $3) RETURNING *;

-- name: DeleteUser :execresult
DELETE FROM users WHERE id = $1;
```

### 3. Generate

```bash
bunx sqlcx-orm generate --sql ./sql --out ./src/db
```

### 4. Use

```ts
import { BunSqlClient } from "./src/db/client";
import { getUser, createUser } from "./src/db/users.queries";

const client = new BunSqlClient(Bun.sql);

const user = await getUser(client, { id: 1 });
// user: { id: number, name: string, email: string, ... } | null

await createUser(client, { name: "Alice", email: "alice@example.com", role: "admin" });
```

## Features

### Inline `@enum` annotations

No separate enum objects. Define values right where the column is:

```sql
-- @enum("draft", "published", "archived")
status TEXT NOT NULL DEFAULT 'draft'
```

Generates `Type.Union([Type.Literal("draft"), Type.Literal("published"), Type.Literal("archived")])` — not a plain `string`.

### Inline `@json` annotations

No more `unknown` for JSON columns:

```sql
-- @json({ theme: string, notifications: boolean, font_size: number })
preferences JSONB
```

Generates a fully typed TypeBox object schema. Supports nested objects, arrays (`string[]`), and nullable (`string?`).

### `RETURNING` clause support

```sql
-- name: CreateUser :one
INSERT INTO users (name, email) VALUES ($1, $2) RETURNING *;
```

Return types are inferred from the table schema — works with `RETURNING *` and explicit column lists.

### `@param` overrides

For ambiguous parameter names:

```sql
-- name: ListByDateRange :many
-- @param $1 start_date
-- @param $2 end_date
SELECT * FROM posts WHERE published_at > $1 AND published_at < $2;
```

### Query commands

| Annotation | Returns | Use for |
|-----------|---------|---------|
| `:one` | `T \| null` | Single row lookups |
| `:many` | `T[]` | List queries |
| `:exec` | `void` | INSERT/UPDATE/DELETE |
| `:execresult` | `{ rowsAffected: number }` | DELETE/UPDATE with count |

### Input/Output type separation

Generated schemas separate Select (all columns) from Insert (defaults are optional):

```ts
// All columns present
export type SelectUsers = { id: number; name: string; created_at: Date; ... }

// Columns with defaults (id, created_at) are Optional
export type InsertUsers = { name: string; email: string; id?: number; created_at?: Date; ... }
```

## CLI

```bash
sqlcx-orm generate [options]   # Parse SQL and generate typed code
sqlcx-orm check [options]      # Validate SQL without generating (CI-friendly)
sqlcx-orm init                 # Scaffold sql/ directory with examples

Options:
  --sql <dir>    SQL directory (default: ./sql)
  --out <dir>    Output directory (default: ./src/db)
  --cache <dir>  Cache directory (default: .sqlcx)
```

## How it works

```
SQL files --> [PostgreSQL Parser] --> IR (Intermediate Representation)
                                      |
                                      +--> [TypeBox Schema Generator] --> schema.ts
                                      +--> [Bun.sql Driver Generator] --> queries.ts + client.ts
```

The IR is language-agnostic and cacheable. Adding new language targets (Python, Go) or schema generators (Zod, Valibot) means implementing a plugin against the IR — the SQL parsing is done once.

## Plugin Architecture

sqlcx is built on four plugin axes:

- **Database Parser** — PostgreSQL (shipped), MySQL/SQLite (planned)
- **Schema Generator** — TypeBox v1.0 (shipped), Zod/Valibot (planned)
- **Driver Generator** — Bun.sql (shipped), pg/mysql2 (planned)
- **Language Plugin** — TypeScript (shipped), Python/Go (planned)

## Why sqlcx?

| Pain point | Prisma | Drizzle | sqlcx |
|-----------|--------|---------|-------|
| JSON columns typed | No (`JsonValue`) | No (`unknown`) | Yes (`@json` annotations) |
| Inline enums | No (separate block) | No (separate object) | Yes (`@enum` annotations) |
| Bundle size | ~1.6MB | ~7.4KB | **0 KB** (no runtime) |
| Multi-language | Dropped Python | TypeScript only | Designed for any language |
| IDE lag on large schemas | Yes (huge .d.ts) | Yes (deep inference) | No (flat interfaces) |
| Generated type bloat | Thousands of lines | Complex generics | Simple flat types |

## License

MIT
