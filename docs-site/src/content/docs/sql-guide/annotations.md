---
title: "@enum / @json / @param"
description: Inline annotations that give sqlcx type hints for columns and query parameters.
---

Annotations are SQL comments placed immediately before the query (or before the relevant column in a schema file). They let you attach type information that sqlcx cannot infer from the database schema alone.

All annotation lines are stripped from the SQL before it reaches the database.

---

## `@enum` — inline enum values

Use `@enum` when a column stores one of a fixed set of string values but there is no `CREATE TYPE … AS ENUM` in the schema. sqlcx generates a union type (TypeScript) or enum (Rust) from the provided values.

### Syntax

```sql
-- @enum("value1", "value2", "value3")
column_name TEXT NOT NULL
```

The annotation must appear on the line immediately before the column name it targets. sqlcx reads the first word of the next non-blank, non-comment line as the column name.

### Example

```sql
-- name: ListPosts :many
SELECT
  id,
  -- @enum("draft", "published", "archived")
  status,
  title
FROM posts;
```

Generated TypeScript:

```typescript
export type Status = "draft" | "published" | "archived";

export type ListPostsRow = {
  id: number;
  status: Status;
  title: string;
};
```

Without the `@enum` annotation, `status` would be typed as `string`.

---

## `@json` — JSON shape for JSONB columns

Use `@json` to describe the runtime shape of a `jsonb` or `json` column. Without it, sqlcx types the column as `unknown` (TypeScript) or `serde_json::Value` (Rust).

### Syntax

```sql
-- @json({ field: type, nested: { a: type } })
column_name JSONB
```

Supported primitive types: `string`, `number`, `boolean`.

Modifiers:
- Array suffix `[]` — e.g. `string[]`
- Nullable suffix `?` — e.g. `string?`

### Example — object shape

```sql
-- name: GetUserSettings :one
SELECT
  id,
  -- @json({ theme: string, fontSize: number, notifications: boolean })
  preferences,
FROM users
WHERE id = $1;
```

Generated TypeScript:

```typescript
export type Preferences = {
  theme: string;
  fontSize: number;
  notifications: boolean;
};

export type GetUserSettingsRow = {
  id: number;
  preferences: Preferences;
};
```

### Example — nested objects

```sql
-- @json({ billing: { plan: string, seats: number }, tags: string[] })
metadata
```

Generated TypeScript:

```typescript
export type Metadata = {
  billing: { plan: string; seats: number };
  tags: string[];
};
```

### Example — nullable and array primitives

```sql
-- @json(string?)
nickname

-- @json(string[])
tags
```

| Annotation | Generated type |
|---|---|
| `@json(string)` | `string` |
| `@json(string?)` | `string \| null` |
| `@json(string[])` | `string[]` |
| `@json(string[]?)` | `string[] \| null` |

---

## `@param` — rename query parameters

By default, sqlcx names parameters `$1`, `$2`, … as `param1`, `param2`, … in generated function signatures. Use `@param` to give them descriptive names.

### Syntax

```sql
-- @param $N descriptive_name
```

`N` is the 1-based parameter index. The name must be a valid identifier (`[a-z_][a-zA-Z0-9_]*`).

### Example

```sql
-- name: SearchUsers :many
-- @param $1 query
-- @param $2 limit
SELECT id, name, email
FROM users
WHERE name ILIKE '%' || $1 || '%'
ORDER BY name
LIMIT $2;
```

Generated TypeScript without annotations:

```typescript
export async function searchUsers(
  sql: Sql,
  param1: string,
  param2: number,
): Promise<SearchUsersRow[]>
```

Generated TypeScript with `@param` annotations:

```typescript
export async function searchUsers(
  sql: Sql,
  query: string,
  limit: number,
): Promise<SearchUsersRow[]>
```

### Multiple parameters

```sql
-- name: GetActivityBetween :many
-- @param $1 start_date
-- @param $2 end_date
SELECT id, user_id, action, created_at
FROM activity_log
WHERE created_at BETWEEN $1 AND $2
ORDER BY created_at;
```

Parameters can be annotated in any order; sqlcx matches by index, not by position of the `@param` line.

---

## Annotation placement rules

- Annotation comments must use the `--` prefix.
- `@enum` and `@json` apply to the **next non-blank, non-comment line** — the column name is read from it automatically.
- `@param` lines can appear anywhere in the comment block before the SQL statement.
- The `-- name:` header must appear before any `@param` annotations.
- Multiple annotations of the same type are allowed (one per column / per parameter index).

---

## Next steps

- [Query Annotations](/sql-guide/query-annotations) — the `-- name:` header format
- [Input/Output Types](/sql-guide/input-output-types) — how insert and select types are generated
- [SELECT * & Partial Select](/sql-guide/select-patterns) — controlling output columns
