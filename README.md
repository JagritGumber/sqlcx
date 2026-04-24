<p align="center">
  <img src="banner.png" width="480" alt="sqlcx" />
</p>

<p align="center">
  <strong>SQL-first. Every language. Zero runtime.</strong>
</p>

<p align="center">
  Write SQL once, generate type-safe code for TypeScript, Python, Go, and Rust.<br/>
  No ORM. No runtime. No engine. Just your SQL and the types that follow.
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/sqlcx-orm"><img src="https://img.shields.io/npm/v/sqlcx-orm?style=flat-square&color=c5d96e" alt="npm" /></a>
  <a href="https://github.com/JagritGumber/sqlcx/blob/main/LICENSE"><img src="https://img.shields.io/github/license/JagritGumber/sqlcx?style=flat-square" alt="MIT" /></a>
</p>

---

## What is sqlcx?

sqlcx reads your SQL schema and annotated queries, then generates fully typed client code — with no runtime library shipped to production.

```
sql/schema.sql        ──┐
sql/queries/users.sql ──┤── sqlcx generate ──┬── schema.ts + users.queries.ts  (TypeScript)
                        │                    ├── models.py                     (Python)
                        │                    ├── models.go + users.queries.go  (Go)
                        │                    └── models.rs + users_queries.rs  (Rust)
                        └────────────────────────────────────────────────────────────────
                                              0 KB runtime
```

## Why not Prisma / Drizzle / sqlc?

| | sqlcx | Prisma | Drizzle | sqlc |
|---|---|---|---|---|
| **Runtime bundle** | **0 KB** | 1.6 MB | 7.4 KB | **0 KB** |
| **TypeScript** | ✓ | ✓ | ✓ | Community |
| **Python** | ✓ (Pydantic + psycopg/asyncpg) | ✓ | — | Community |
| **Go** | ✓ | — | — | ✓ |
| **Rust** | ✓ | — | — | — |
| **Drivers** | **10** (4 TS, 2 Py, 2 Go, 2 Rust) | 1 | 1 | 1 |
| **Validation** | TypeBox, Zod, Pydantic, Serde | Built-in | Built-in | — |
| **Multi-language** | ✓ (one SQL, all targets) | — | — | Go only |

**sqlcx** = the sqlc model (SQL-first, zero runtime) but for every language.

---

## Quick Start

### Install

```bash
npm install sqlcx-orm
# or
cargo install sqlcx
```

### 1. Write your schema

```sql
-- sql/schema.sql
CREATE TYPE user_status AS ENUM ('active', 'inactive', 'banned');

CREATE TABLE users (
  id         SERIAL      PRIMARY KEY,
  name       TEXT        NOT NULL,
  email      TEXT        NOT NULL UNIQUE,
  bio        TEXT,
  status     user_status NOT NULL DEFAULT 'active',
  tags       TEXT[],
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### 2. Write annotated queries

```sql
-- sql/queries/users.sql

-- name: GetUser :one
SELECT * FROM users WHERE id = $1;

-- name: ListUsers :many
SELECT id, name, email, status FROM users ORDER BY created_at DESC;

-- name: CreateUser :exec
INSERT INTO users (name, email) VALUES ($1, $2);

-- name: DeleteUser :execresult
DELETE FROM users WHERE id = $1;
```

### 3. Configure

```toml
# sqlcx.toml
sql    = "./sql"
parser = "postgres"

[[targets]]
language = "typescript"
out      = "src/generated"
schema   = "typebox"
driver   = "bun-sql"

[[targets]]
language = "python"
out      = "py/generated"
schema   = "pydantic"
driver   = "psycopg"
```

### 4. Generate

```bash
npx sqlcx generate
```

### 5. Use

**TypeScript:**
```typescript
import { getUser, listUsers, createUser } from './generated/users.queries';

const user = await getUser(sql, { id: 42 });
console.log(user.name);    // string
console.log(user.status);  // "active" | "inactive" | "banned"
```

**Python (psycopg):**
```python
from generated.users_queries import get_user, list_users, create_user

user = get_user(conn, GetUserParams(id=42))
print(user.name)    # str
print(user.status)  # str
```

**Go:**
```go
user, err := queries.GetUser(ctx, 42)
fmt.Println(user.Name, user.Email)
```

**Rust:**
```rust
let user = get_user(&pool, 42).await?;
println!("{} <{}>", user.name, user.email);
```

Every version is fully typed from your SQL. No hand-written interfaces. No `any`.

---

## Migrations

sqlcx ships with an optional built-in migrator for PostgreSQL. Schema changes and typed clients stay in sync automatically: after every `migrate up`, sqlcx re-runs the codegen so your types reflect the new shape of the database.

### Configure

Add a `[migrate]` section to `sqlcx.toml` (or run `sqlcx init` which scaffolds this for you):

```toml
[migrate]
dir             = "./sql/migrations"
auto_regenerate = true
# database_url  = "postgres://user:pass@localhost:5432/mydb"
```

If `database_url` is not set in config, sqlcx reads `SQLCX_DATABASE_URL` from your environment. Keep secrets out of version control by preferring the env var.

### Workflow

```bash
# Create a new timestamped migration file in sql/migrations/
sqlcx migrate new create_users

# Edit the generated file, then apply all pending migrations
sqlcx migrate up

# See which migrations are pending, applied, or drifted
sqlcx migrate status
```

Each `migrate new` creates a file named `{YYYYMMDDHHMMSS}_{name}.sql`. You write plain SQL inside — no annotations, no framework-specific syntax. sqlcx tracks applied state in a `_sqlcx_migrations` table it creates on first run.

### Drift detection

sqlcx stores a SHA-256 checksum of every migration when it is applied. If a file is edited after it has been applied to the database, `migrate up` and `migrate status` will report **DRIFTED** on that version and refuse to apply new migrations until it is resolved. This catches the common "someone edited an old migration" footgun before it corrupts your schema history.

### Auto-regenerate

When `auto_regenerate = true` (the default), a successful `migrate up` automatically runs the codegen pipeline. Your typed clients stay in lockstep with the database without any manual re-run of `sqlcx generate`.

### Cargo feature

The migrator is gated behind the `migrate` Cargo feature (enabled by default for `sqlcx` the CLI binary). Library consumers of `sqlcx-core` who want pure codegen and zero database dependencies can opt out with `default-features = false`.

---

## Features

### Multi-language from one SQL source

Write SQL once. Generate TypeScript, Python, Go, and Rust from the same schema and queries. Perfect for polyglot backends, microservice architectures, or gradual language migrations.

### Zero runtime

sqlcx generates code at build time. The output imports only your database driver. There is nothing between your query and the wire — no engine, no proxy, no runtime library.

### Inline `@enum` annotations

No separate enum objects needed. Define values right where the column is:

```sql
-- @enum("draft", "published", "archived")
status TEXT NOT NULL DEFAULT 'draft'
```

Generates a proper union type (TypeScript), `str(Enum)` class (Python), or string constant (Go/Rust) — not a plain `string`.

### Inline `@json` annotations

No more `unknown` / `Any` for JSON columns:

```sql
-- @json({ theme: string, notifications: boolean, font_size: number })
preferences JSONB
```

Generates a fully typed schema. Supports nested objects, arrays (`string[]`), and nullable (`string?`).

### `@param` — named parameters

Give your query parameters descriptive names:

```sql
-- name: SearchUsers :many
-- @param $1 query
-- @param $2 limit
SELECT id, name, email FROM users
WHERE name ILIKE '%' || $1 || '%'
LIMIT $2;
```

### Query commands

| Annotation | Returns | Use for |
|-----------|---------|---------|
| `:one` | Single row or null | `SELECT ... WHERE id = $1` |
| `:many` | Array of rows | `SELECT ...` without unique filter |
| `:exec` | Nothing | `INSERT`, `UPDATE`, `DELETE` |
| `:execresult` | Affected row count | Mutations where count matters |

### Select / Insert type separation

Every table gets two types — one for reading, one for writing:

```python
# Python (Pydantic)
class SelectUsers(BaseModel):      # All columns present
    id: int
    name: str
    status: UserStatus
    created_at: datetime

class InsertUsers(BaseModel):      # Defaults are optional
    name: str
    email: str
    id: int | None = None
    status: UserStatus | None = None
    created_at: datetime | None = None
```

### Partial column selection

Only select the columns you need — the generated type matches exactly:

```sql
-- name: ListUserEmails :many
SELECT id, email FROM users;
```

Generates `ListUserEmailsRow` with only `{ id, email }` — not the full table type.

### Current query boundary

sqlcx currently supports single-table query shape inference for generated row types and parameter typing, including qualified references to the base table.

- `SELECT * FROM users`
- `SELECT id, email FROM users`
- `SELECT users.id, users.name AS user_name FROM users`
- `INSERT ... VALUES (...)`
- `UPDATE ... RETURNING id, name`

Join-shaped projections such as `SELECT users.id, orgs.slug ...` are still rejected for now instead of generating invalid code. That keeps the generated output sound while the multi-table IR is still intentionally narrow.

### Caching

sqlcx hashes your SQL files together with the active parser. If nothing relevant changed, parsing is skipped entirely. Subsequent runs are near-instant.

```bash
# First run: parses SQL
$ npx sqlcx generate    # ~200ms

# Second run: cached
$ npx sqlcx generate    # ~20ms
```

---

## Supported Targets

### Languages & Schema Generators

| Language | Schema | Output |
|----------|--------|--------|
| TypeScript | `typebox` | TypeBox validators + static types |
| TypeScript | `zod` | Zod v4 schemas |
| TypeScript | `zod/v3` | Zod v3 schemas |
| Python | `pydantic` | Pydantic v2 BaseModel classes |
| Go | `structs` | Go structs with `db`/`json` tags |
| Rust | `serde` | Serde + sqlx::FromRow structs |

### Database Drivers

| Language | Driver | Description |
|----------|--------|-------------|
| TypeScript | `bun-sql` | Typed functions for Bun's built-in SQL |
| TypeScript | `pg` | Typed functions for node-postgres |
| TypeScript | `mysql2` | Typed functions for mysql2 (MySQL) |
| TypeScript | `better-sqlite3` | Typed synchronous functions for better-sqlite3 (SQLite) |
| Python | `psycopg` | Typed functions for psycopg3 (sync Postgres) |
| Python | `asyncpg` | Typed async functions for asyncpg (async Postgres) |
| Go | `database-sql` | Typed functions for `database/sql` |
| Go | `pgx` | Typed functions for jackc/pgx v5 (modern Postgres) |
| Rust | `sqlx` | Typed async functions for sqlx |
| Rust | `tokio-postgres` | Typed async functions for tokio-postgres |

### Database Parsers

| Parser | Features |
|--------|----------|
| `postgres` | ENUMs, arrays, JSONB, UUID, `$1` params |
| `mysql` | Inline ENUMs, `TINYINT(1)` booleans, `AUTO_INCREMENT`, `?` params |
| `sqlite` | Type affinity mapping, `AUTOINCREMENT`, `?` params |

---

## CLI

```bash
npx sqlcx generate    # Parse SQL → generate typed code
npx sqlcx check       # Validate SQL without generating (CI-friendly)
npx sqlcx init        # Scaffold sql/ directory + sqlcx.toml
npx sqlcx schema      # Emit JSON Schema for config validation
```

---

## Configuration

**`sqlcx.toml`** — minimal:

```toml
sql    = "./sql"
parser = "postgres"

[[targets]]
language = "typescript"
out      = "src/generated"
schema   = "typebox"
driver   = "bun-sql"
```

**Multi-target** — generate all languages at once:

```toml
sql    = "./sql"
parser = "postgres"

[[targets]]
language = "typescript"
out      = "src/generated"
schema   = "typebox"
driver   = "bun-sql"

[[targets]]
language = "python"
out      = "py/generated"
schema   = "pydantic"
driver   = "asyncpg"

[[targets]]
language = "go"
out      = "internal/db"
schema   = "structs"
driver   = "pgx"

[[targets]]
language = "rust"
out      = "src/db"
schema   = "serde"
driver   = "sqlx"
```

**Type overrides:**

```toml
[overrides]
uuid = "string"    # Map UUID to string in all targets
```

---

## Architecture

```
SQL files ──▶ Parser (postgres/mysql/sqlite) ──▶ IR (tables, queries, enums)
                                                    │
                                                    ├──▶ TypeScript Plugin ──▶ schema.ts + queries.ts
                                                    │    (bun-sql, pg, mysql2, better-sqlite3)
                                                    ├──▶ Python Plugin    ──▶ models.py + queries.py
                                                    │    (psycopg, asyncpg)
                                                    ├──▶ Go Plugin        ──▶ models.go + queries.go
                                                    │    (database-sql, pgx)
                                                    └──▶ Rust Plugin      ──▶ models.rs + queries.rs
                                                         (sqlx, tokio-postgres)
```

The IR (Intermediate Representation) is language-agnostic and cacheable. Each language plugin consumes the same IR and produces idiomatic output for its ecosystem.

Adding a new language = implementing `SchemaGenerator` + `DriverGenerator` traits against the IR.

---

## Project Structure

```
sqlcx/
├── crates/
│   ├── sqlcx-core/          # Core library
│   │   └── src/
│   │       ├── parser/      # SQL parsers (postgres, mysql, sqlite)
│   │       ├── generator/   # Language plugins
│   │       │   ├── typescript/  # TypeBox, Zod, Bun SQL, pg, mysql2, better-sqlite3
│   │       │   ├── python/      # Pydantic, psycopg, asyncpg
│   │       │   ├── go/          # Structs, database/sql, pgx
│   │       │   └── rust_lang/   # Serde, sqlx, tokio-postgres
│   │       ├── ir.rs        # Intermediate representation
│   │       └── config.rs    # Config parsing
│   └── sqlcx/               # CLI binary
├── packages/
│   └── js/                  # npm binary distribution
├── docs/                    # Documentation site (Astro + Starlight)
└── tests/                   # Integration tests + fixtures
```

---

## Contributing

Contributions welcome. The best way to get started:

1. Look at an existing generator (e.g., `generator/typescript/typebox.rs`)
2. The pattern is the same for every language: implement `SchemaGenerator` and optionally `DriverGenerator`
3. Run `cargo test` to verify

---

## License

MIT
