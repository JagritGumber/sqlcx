<p align="center">
  <img src="banner.png" width="480" alt="sqlcx" />
</p>

<p align="center">
  <strong>Typed SQL for every language, without forcing a new runtime.</strong>
</p>

<p align="center">
  Write SQL once, generate type-safe code for TypeScript, Python, Go, and Rust.<br/>
  sqlcx is codegen. It works with your existing drivers and database engines.
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/sqlcx-orm"><img src="https://img.shields.io/npm/v/sqlcx-orm?style=flat-square&color=c5d96e" alt="npm" /></a>
  <a href="https://github.com/JagritGumber/sqlcx/blob/main/LICENSE"><img src="https://img.shields.io/github/license/JagritGumber/sqlcx?style=flat-square" alt="MIT" /></a>
</p>

---

## What is sqlcx?

sqlcx reads your SQL schema and annotated queries, then generates typed client code.

It is not a database.
It is not an ORM runtime.
It is not a proxy.

sqlcx is the cross-engine, cross-language codegen layer.

- sqlcx = SQL-first codegen for apps
- evolvsql = the optional adaptive engine vision behind the scenes

That distinction matters. sqlcx should work with normal databases and drivers even if you never use evolvsql.

If you want the longer-term engine direction, read [docs/evolvsql.md](docs/evolvsql.md).

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
| **Python** | ✓ (Pydantic + psycopg/asyncpg/sqlite3/mysql) | ✓ | — | Community |
| **Go** | ✓ | — | — | ✓ |
| **Rust** | ✓ | — | — | — |
| **Drivers** | **12** (4 TS, 4 Py, 2 Go, 2 Rust) | 1 | 1 | 1 |
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

## Features

### Multi-language from one SQL source

Write SQL once. Generate TypeScript, Python, Go, and Rust from the same schema and queries. Perfect for polyglot backends, microservice architectures, or gradual language migrations.

### Zero required runtime

sqlcx generates code at build time. The output imports only your database driver. There is nothing mandatory between your query and the wire: no proxy, no required runtime library, and no forced engine swap.

If you want adaptive behavior later, that belongs in evolvsql. sqlcx itself stays usable with the database stack you already have.

### sqlcx vs evolvsql

sqlcx is the developer-facing product: typed SQL, codegen, and multi-language clients.

evolvsql is the longer-term adaptive layer: the part that can learn from query patterns, schema evolution, and application behavior over time.

In short:

- use sqlcx when you want typed SQL and codegen today
- use evolvsql when the adaptive engine vision is ready
- do not frame either one as "better Postgres"

### Inline `@enum` annotations

No separate enum objects needed. Define values right where the column is:

```sql
-- @enum("draft", "published", "archived")
status TEXT NOT NULL DEFAULT 'draft'
```

Generates a proper union type (TypeScript), `str(Enum)` class (Python), or string constant (Go/Rust), not a plain `string`.

### Inline `@json` annotations

No more `unknown` / `Any` for JSON columns:

```sql
-- @json({ theme: string, notifications: boolean, font_size: number })
preferences JSONB
```

Generates a fully typed schema. Supports nested objects, arrays (`string[]`), and nullable (`string?`).

### `@param` named parameters

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

Every table gets two types: one for reading, one for writing.

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

Only select the columns you need. The generated type matches exactly.

```sql
-- name: ListUserEmails :many
SELECT id, email FROM users;
```

Generates `ListUserEmailsRow` with only `{ id, email }`, not the full table type.

### Caching

sqlcx hashes your SQL files. If nothing changed, parsing is skipped entirely. Subsequent runs are near-instant.

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
| Python | `sqlite3` | Typed functions for Python's built-in sqlite3 |
| Python | `mysql-connector` | Typed functions for mysql-connector-python (MySQL) |
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
npx sqlcx schema      # Emit JSON Schema for config validation (coming soon)
```

---

## Configuration

**`sqlcx.toml`** minimal:

```toml
sql    = "./sql"
parser = "postgres"

[[targets]]
language = "typescript"
out      = "src/generated"
schema   = "typebox"
driver   = "bun-sql"
```

**Multi-target** generate all languages at once:

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
