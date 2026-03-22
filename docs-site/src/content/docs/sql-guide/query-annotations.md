---
title: Query Annotations
description: The -- name annotation format that tells sqlcx how to name a query and what it returns.
---

Every SQL query that sqlcx processes must start with a name annotation. This single comment line tells sqlcx the query's name and how to wrap its return value.

---

## Syntax

```sql
-- name: QueryName :command
```

- `QueryName` — PascalCase identifier used as the generated function name
- `:command` — one of `one`, `many`, `exec`, `execresult`

The annotation must be the first non-blank line of the query block. Annotation lines are stripped from the SQL before it is sent to the database.

---

## Command types

| Command | Returns | Typical usage |
|---|---|---|
| `:one` | Single row or `null` | `SELECT … WHERE id = $1` |
| `:many` | Array of rows (may be empty) | `SELECT …` without unique filter |
| `:exec` | Nothing / void | `INSERT`, `UPDATE`, `DELETE` |
| `:execresult` | Affected row count | `UPDATE` or `DELETE` where count matters |

---

## `:one` — single row or null

Use `:one` when the query is expected to return at most one row. The generated function returns the row type or `null` (TypeScript) / `Option<T>` (Rust).

```sql
-- name: GetUser :one
SELECT id, name, email
FROM users
WHERE id = $1;
```

Generated TypeScript:

```typescript
export async function getUser(
  sql: Sql,
  id: number,
): Promise<GetUserRow | null> { … }
```

---

## `:many` — array of rows

Use `:many` for queries that can return zero or more rows. The generated function always returns an array.

```sql
-- name: ListActiveUsers :many
SELECT id, name, email
FROM users
WHERE status = 'active'
ORDER BY name;
```

Generated TypeScript:

```typescript
export async function listActiveUsers(
  sql: Sql,
): Promise<ListActiveUsersRow[]> { … }
```

---

## `:exec` — void

Use `:exec` for `INSERT`, `UPDATE`, or `DELETE` statements where you do not need to know how many rows were affected. The generated function returns `void` / `()`.

```sql
-- name: DeleteUser :exec
DELETE FROM users
WHERE id = $1;
```

Generated TypeScript:

```typescript
export async function deleteUser(
  sql: Sql,
  id: number,
): Promise<void> { … }
```

---

## `:execresult` — affected row count

Use `:execresult` when you need to know how many rows the statement touched. The generated function returns a number.

```sql
-- name: DeactivateStaleUsers :execresult
UPDATE users
SET status = 'inactive'
WHERE last_seen_at < $1;
```

Generated TypeScript:

```typescript
export async function deactivateStaleUsers(
  sql: Sql,
  lastSeenAt: Date,
): Promise<number> { … }
```

---

## Naming conventions

Query names must be PascalCase identifiers (`[A-Z][a-zA-Z0-9]*`). sqlcx converts them to camelCase for the generated function name:

| Annotation name | Generated function |
|---|---|
| `GetUser` | `getUser` |
| `ListActiveUsers` | `listActiveUsers` |
| `DeactivateStaleUsers` | `deactivateStaleUsers` |

---

## Next steps

- [@enum / @json / @param](/sql-guide/annotations) — add type hints for columns and parameters
- [Input/Output Types](/sql-guide/input-output-types) — how select and insert types are derived
- [SELECT * & Partial Select](/sql-guide/select-patterns) — controlling which columns appear in output types
