---
title: "SELECT * & Partial Select"
description: How SELECT * and partial selects affect the types sqlcx generates.
---

The columns you name in a `SELECT` clause directly determine the shape of the generated row type. sqlcx never guesses — it derives types from exactly what the query returns.

---

## `SELECT *` — full table expansion

`SELECT *` expands to every column in the table (or joined tables). sqlcx generates a row type with a field for each column, in schema-definition order.

```sql
-- name: GetUser :one
SELECT * FROM users WHERE id = $1;
```

Given this schema:

```sql
CREATE TABLE users (
  id         SERIAL    PRIMARY KEY,
  name       TEXT      NOT NULL,
  email      TEXT      NOT NULL,
  bio        TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
```

sqlcx generates:

```typescript
export type GetUserRow = {
  id: number;
  name: string;
  email: string;
  bio: string | null;
  created_at: Date;
};
```

`SELECT *` is convenient but couples generated types to the full table shape. Adding a column to the table changes every `SELECT *` type automatically.

---

## Partial select — named columns

Listing specific columns produces a type with only those fields. This is useful when you want a lighter result shape or when joining tables would create name collisions.

```sql
-- name: ListUserSummaries :many
SELECT id, name, email
FROM users
ORDER BY name;
```

Generated type — only the three named columns:

```typescript
export type ListUserSummariesRow = {
  id: number;
  name: string;
  email: string;
};
```

`bio` and `created_at` are absent because they were not selected.

---

## Column aliasing

Use `AS` to rename a column in the output. The generated field name matches the alias, not the original column name.

```sql
-- name: GetUserProfile :one
SELECT
  u.id,
  u.name    AS username,
  u.email   AS contact_email,
  p.bio     AS profile_bio
FROM users u
LEFT JOIN profiles p ON p.user_id = u.id
WHERE u.id = $1;
```

Generated type:

```typescript
export type GetUserProfileRow = {
  id: number;
  username: string;
  contact_email: string;
  profile_bio: string | null;
};
```

Aliases are required when joining two tables that have columns with the same name, since the field names in the result type must be unique.

---

## Expressions and computed columns

Expressions in the `SELECT` clause must have an alias; otherwise sqlcx cannot name the field.

```sql
-- name: GetUserStats :one
SELECT
  id,
  LOWER(email)          AS email_lower,
  EXTRACT(YEAR FROM created_at) AS join_year
FROM users
WHERE id = $1;
```

Generated type:

```typescript
export type GetUserStatsRow = {
  id: number;
  email_lower: string;
  join_year: number;
};
```

---

## `SELECT *` with enum columns

When a column has a `CREATE TYPE … AS ENUM` in the schema, `SELECT *` picks it up and sqlcx generates the proper union type — no `@enum` annotation needed.

```sql
CREATE TYPE post_status AS ENUM ('draft', 'published', 'archived');

CREATE TABLE posts (
  id     SERIAL      PRIMARY KEY,
  title  TEXT        NOT NULL,
  status post_status NOT NULL DEFAULT 'draft'
);
```

```sql
-- name: ListPosts :many
SELECT * FROM posts;
```

Generated:

```typescript
export type PostStatus = "draft" | "published" | "archived";

export type ListPostsRow = {
  id: number;
  title: string;
  status: PostStatus;   // union, not string
};
```

For columns without a `CREATE TYPE` (e.g. a plain `TEXT` column with known values), use `@enum` to get the same effect. See [@enum / @json / @param](/sql-guide/annotations).

---

## Choosing between `SELECT *` and partial select

| Scenario | Recommendation |
|---|---|
| Need all columns, types change with schema | `SELECT *` |
| Joining multiple tables | Partial select with aliases |
| Performance-sensitive query | Partial select (fewer columns transferred) |
| Returning a stable public API shape | Partial select (schema changes won't break callers) |
| Quick internal helper | Either — `SELECT *` is fine |

---

## Next steps

- [Query Annotations](/sql-guide/query-annotations) — the `-- name:` header format
- [@enum / @json / @param](/sql-guide/annotations) — type hints for columns and parameters
- [Input/Output Types](/sql-guide/input-output-types) — select types vs insert types
