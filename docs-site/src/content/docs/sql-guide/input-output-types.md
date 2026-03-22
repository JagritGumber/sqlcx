---
title: Input/Output Types
description: How sqlcx derives select (output) and insert (input) types from your schema.
---

sqlcx generates two distinct type shapes for every table: one for reading rows out of the database and one for writing rows in. Understanding the difference prevents confusion when building insert/update helpers.

---

## Select types (output)

A select type represents a row returned by a query. It includes **every column** in the query result, exactly as the database returns it.

- `NOT NULL` columns are non-optional.
- Nullable columns become `T | null` (TypeScript) or `Option<T>` (Rust).
- Columns with database-managed values (`SERIAL`, `DEFAULT`, `GENERATED`) are still present — the row already has those values when it comes back.

```sql
CREATE TABLE users (
  id         SERIAL      PRIMARY KEY,
  name       TEXT        NOT NULL,
  email      TEXT        NOT NULL,
  bio        TEXT,                        -- nullable
  status     TEXT        NOT NULL DEFAULT 'active',
  created_at TIMESTAMP   NOT NULL DEFAULT NOW()
);
```

```typescript
// Generated select type — all columns present
export type UsersSelect = {
  id: number;          // SERIAL — always present in results
  name: string;
  email: string;
  bio: string | null;  // nullable column
  status: string;      // has DEFAULT but still returned
  created_at: Date;    // has DEFAULT but still returned
};
```

---

## Insert types (input)

An insert type represents the data you supply when inserting a row. Columns that the database fills in automatically are **excluded or made optional** so you cannot accidentally pass them (or so TypeScript does not force you to).

Columns omitted or made optional in insert types:

| Column kind | Reason | Behavior |
|---|---|---|
| `SERIAL` / `BIGSERIAL` | Auto-incremented by the database | Omitted entirely |
| `GENERATED ALWAYS` | Computed by the database | Omitted entirely |
| `DEFAULT <expr>` | Database supplies a fallback | Made optional (`?`) |
| Nullable (no `DEFAULT`) | Can be `NULL` — caller may omit | Made optional |

Using the same `users` table:

```typescript
// Generated insert type — caller only provides what is needed
export type UsersInsert = {
  // id is omitted — SERIAL, database assigns it
  name: string;          // required, no default
  email: string;         // required, no default
  bio?: string | null;   // nullable — optional to provide
  status?: string;       // has DEFAULT 'active' — optional
  // created_at is omitted — has DEFAULT NOW()
};
```

---

## Side-by-side comparison

```sql
CREATE TABLE posts (
  id         SERIAL    PRIMARY KEY,
  author_id  INTEGER   NOT NULL,
  title      TEXT      NOT NULL,
  body       TEXT,
  published  BOOLEAN   NOT NULL DEFAULT false,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
```

| Column | UsersSelect | UsersInsert |
|---|---|---|
| `id` | `number` | omitted (SERIAL) |
| `author_id` | `number` | `number` (required) |
| `title` | `string` | `string` (required) |
| `body` | `string \| null` | `string \| null \| undefined` (optional) |
| `published` | `boolean` | `boolean \| undefined` (has DEFAULT) |
| `created_at` | `Date` | omitted (has DEFAULT) |

---

## How these map to generated functions

Select types appear as **return types** on query functions:

```typescript
// :one query — returns select type or null
export async function getPost(sql: Sql, id: number): Promise<PostsSelect | null>

// :many query — returns array of select type
export async function listPosts(sql: Sql): Promise<PostsSelect[]>
```

Insert types appear as **parameter types** on insert functions:

```typescript
// :exec insert — takes insert type as input
export async function createPost(sql: Sql, args: PostsInsert): Promise<void>
```

---

## Partial selects

When you write a query that selects only some columns, sqlcx generates a narrower type containing only those columns rather than reusing the full select type. See [SELECT * & Partial Select](/sql-guide/select-patterns) for details.

---

## Next steps

- [Query Annotations](/sql-guide/query-annotations) — the `-- name:` header
- [@enum / @json / @param](/sql-guide/annotations) — add type hints for columns
- [SELECT * & Partial Select](/sql-guide/select-patterns) — control which columns appear in output types
