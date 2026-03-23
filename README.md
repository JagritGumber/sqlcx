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
  <a href="https://github.com/JagritGumber/sqlcx/actions"><img src="https://img.shields.io/github/actions/workflow/status/JagritGumber/sqlcx/ci.yml?style=flat-square" alt="CI" /></a>
  <a href="https://www.npmjs.com/package/sqlcx-orm"><img src="https://img.shields.io/npm/v/sqlcx-orm?style=flat-square&color=c5d96e" alt="npm" /></a>
  <a href="https://crates.io/crates/sqlcx"><img src="https://img.shields.io/crates/v/sqlcx?style=flat-square&color=c5d96e" alt="crates.io" /></a>
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
| **Python** | ✓ (Pydantic) | ✓ | — | Community |
| **Go** | ✓ | — | — | ✓ |
| **Rust** | ✓ | — | — | — |
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
driver   = "none"
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

**Python:**
```python
from generated.models import SelectUsers, UserStatus

user = SelectUsers(id=1, name="Alice", email="alice@example.com",
                   status=UserStatus.ACTIVE, created_at=datetime.now())
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
| Go | `database-sql` | Typed functions for `database/sql` |
| Rust | `sqlx` | Typed async functions for sqlx |
| Python | `none` | Schema-only (driver coming soon) |

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
npx sqlcx init        # Scaffold sql/ directory + sqlcx.toml (coming soon)
npx sqlcx schema      # Emit JSON Schema for config validation (coming soon)
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
driver   = "none"

[[targets]]
language = "go"
out      = "internal/db"
schema   = "structs"
driver   = "database-sql"

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
                                                    ├──▶ Python Plugin    ──▶ models.py
                                                    ├──▶ Go Plugin        ──▶ models.go + queries.go
                                                    └──▶ Rust Plugin      ──▶ models.rs + queries.rs
```

The IR (Intermediate Representation) is language-agnostic and cacheable. Each language plugin consumes the same IR and produces idiomatic output for its ecosystem.

Adding a new language = implementing `SchemaGenerator` + `DriverGenerator` traits against the IR.

---

## Project Structure

```
sqlcx-rust/
├── crates/
│   ├── sqlcx-core/          # Core library
│   │   └── src/
│   │       ├── parser/      # SQL parsers (postgres, mysql, sqlite)
│   │       ├── generator/   # Language plugins
│   │       │   ├── typescript/  # TypeBox, Zod, Bun SQL, pg
│   │       │   ├── python/      # Pydantic
│   │       │   ├── go/          # Structs, database/sql
│   │       │   └── rust_lang/   # Serde, sqlx
│   │       ├── ir.rs        # Intermediate representation
│   │       └── config.rs    # Config parsing
│   └── sqlcx/               # CLI binary
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
