---
title: pg Driver
description: Generate type-safe query functions for node-postgres (pg).
---

sqlcx generates a `client.ts` adapter and one `.queries.ts` file per SQL query file when `driver = "pg"` is set. The generated functions accept a `DatabaseClient` interface — a thin wrapper around a `pg.Pool` that normalises the query, queryOne, and execute patterns.

---

## Configuration

```toml
[[targets]]
language = "typescript"
out      = "src/generated"
schema   = "sql/schema.sql"
driver   = "pg"
```

Install the `pg` package:

```bash
npm install pg
npm install --save-dev @types/pg
```

---

## Generated files

Running `sqlcx generate` with this driver produces:

```
src/generated/
  client.ts         — DatabaseClient interface + PgClient adapter
  users.queries.ts  — typed functions for each query in users.sql
```

---

## Generated client.ts

sqlcx always emits a `client.ts` with the `DatabaseClient` interface and a `PgClient` adapter:

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

`PgClient` wraps a `Pool`. All queries go through `pool.query(text, values)` using pg's parameterised query format. `result.rowCount` carries affected row counts for `INSERT`/`UPDATE`/`DELETE` — it can be `null` for non-DML statements, so the adapter falls back to `0`.

---

## Generated query functions

Given these queries in `sql/queries/users.sql`:

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

sqlcx generates `users.queries.ts`:

```typescript
import type { DatabaseClient } from "./client";

export interface GetUserRow {
  id: number;
  name: string;
  email: string;
  bio: string | null;
  status: string;
  tags: string[] | null;
  created_at: Date;
}

export interface GetUserParams {
  id: number;
}

export const getUserSql = "SELECT * FROM users WHERE id = $1";

export async function getUser(client: DatabaseClient, params: GetUserParams): Promise<GetUserRow | null> {
  return client.queryOne<GetUserRow>(getUserSql, [params.id]);
}

export interface ListUsersRow {
  id: number;
  name: string;
  email: string;
}

export interface ListUsersParams {
  name: string;
}

export const listUsersSql = "SELECT id, name, email FROM users WHERE name ILIKE $1";

export async function listUsers(client: DatabaseClient, params: ListUsersParams): Promise<ListUsersRow[]> {
  return client.query<ListUsersRow>(listUsersSql, [params.name]);
}

export interface CreateUserParams {
  name: string;
  email: string;
  bio: string;
}

export const createUserSql = "INSERT INTO users (name, email, bio) VALUES ($1, $2, $3)";

export async function createUser(client: DatabaseClient, params: CreateUserParams): Promise<void> {
  await client.execute(createUserSql, [params.name, params.email, params.bio]);
}

export interface DeleteUserParams {
  id: number;
}

export const deleteUserSql = "DELETE FROM users WHERE id = $1";

export async function deleteUser(client: DatabaseClient, params: DeleteUserParams): Promise<{ rowsAffected: number }> {
  return client.execute(deleteUserSql, [params.id]);
}
```

---

## Return types by command

| SQL annotation | Return type | How it works |
|---------------|-------------|--------------|
| `:one` | `Promise<T \| null>` | Returns `result.rows[0] ?? null` |
| `:many` | `Promise<T[]>` | Returns `result.rows` |
| `:exec` | `Promise<void>` | Calls `execute`, discards result |
| `:execresult` | `Promise<{ rowsAffected: number }>` | Returns `result.rowCount ?? 0` |

---

## Usage

```typescript
import { Pool } from "pg";
import { PgClient } from "./generated/client";
import { getUser, listUsers, createUser, deleteUser } from "./generated/users.queries";

const pool = new Pool({ connectionString: process.env.DATABASE_URL });
const client = new PgClient(pool);

// :one — returns the row or null
const user = await getUser(client, { id: 1 });
if (user) {
  console.log(user.name, user.status);
}

// :many — returns an array
const results = await listUsers(client, { name: "%alice%" });
for (const row of results) {
  console.log(row.email);
}

// :exec — fire and forget
await createUser(client, { name: "Alice", email: "alice@example.com", bio: null });

// :execresult — get affected row count
const { rowsAffected } = await deleteUser(client, { id: 1 });
console.log(`Deleted ${rowsAffected} row(s)`);
```

---

## Connection pooling

`PgClient` takes a `Pool`, not a `Client`. Using a pool is strongly recommended in production: pg manages connection acquisition and release internally, and all generated functions are stateless — they acquire a connection per query and release it immediately.

For transaction support, acquire a client directly from the pool and pass it wrapped in a `DatabaseClient`-compatible adapter.

---

## Next steps

- [Bun SQL](/typescript/bun-sql) — use Bun's built-in SQL driver instead
- [TypeBox](/typescript/typebox) — add runtime validation to your generated types
- [Zod](/typescript/zod) — use Zod schemas with your generated types
