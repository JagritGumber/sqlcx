---
title: TypeBox
description: Generate TypeBox schemas from your SQL tables with full static type inference.
---

import { Tabs, TabItem } from '@astrojs/starlight/components';

sqlcx generates a `schema.ts` file containing TypeBox validators for every table in your schema. Each validator is a `Type.Object()` with field-level types derived directly from your SQL column definitions — no manual type mapping required.

---

## Configuration

Set `schema = "typebox"` in your target block:

```toml
[[targets]]
language = "typescript"
out      = "src/generated"
schema   = "sql/schema.sql"
driver   = "bun-sql"

[targets.schema_options]
schema = "typebox"
```

Install the TypeBox package:

```bash
npm install @sinclair/typebox
# requires @sinclair/typebox >= 0.31.0
```

---

## Generated output

Given this schema:

```sql
CREATE TYPE user_status AS ENUM ('active', 'inactive', 'banned');

CREATE TABLE users (
  id         SERIAL      PRIMARY KEY,
  name       TEXT        NOT NULL,
  bio        TEXT,
  status     user_status NOT NULL DEFAULT 'active',
  tags       TEXT[],
  created_at TIMESTAMP   NOT NULL DEFAULT NOW()
);
```

sqlcx writes `schema.ts`:

```typescript
import { Type, type Static } from "@sinclair/typebox";

// Requires @sinclair/typebox >= 0.31.0 (for Type.Date and Type.Uint8Array)

type Prettify<T> = { [K in keyof T]: T[K] } & {};

export const UserStatus = Type.Union([Type.Literal("active"), Type.Literal("inactive"), Type.Literal("banned")]);

export const SelectUsers = Type.Object({
  "id": Type.Number(),
  "name": Type.String(),
  "bio": Type.Union([Type.String(), Type.Null()]),
  "status": UserStatus,
  "tags": Type.Union([Type.Array(Type.String()), Type.Null()]),
  "created_at": Type.Date()
});

export const InsertUsers = Type.Object({
  "id": Type.Optional(Type.Number()),
  "name": Type.String(),
  "bio": Type.Optional(Type.Union([Type.String(), Type.Null()])),
  "status": Type.Optional(UserStatus),
  "tags": Type.Optional(Type.Union([Type.Array(Type.String()), Type.Null()])),
  "created_at": Type.Optional(Type.Date())
});

export type SelectUsers = Prettify<Static<typeof SelectUsers>>;
export type InsertUsers = Prettify<Static<typeof InsertUsers>>;
export type UserStatus = Prettify<Static<typeof UserStatus>>;
```

---

## How types are derived

### Select schemas

Each column maps to a TypeBox type. Nullable columns are wrapped in `Type.Union([..., Type.Null()])`:

```typescript
// NOT NULL column
"name": Type.String()

// Nullable column
"bio": Type.Union([Type.String(), Type.Null()])
```

### Insert schemas

Columns with a `DEFAULT` value or that are nullable become `Type.Optional(...)`:

```typescript
// Required, no default
"name": Type.String()

// Has DEFAULT — optional on insert
"status": Type.Optional(UserStatus)

// Nullable — optional on insert
"bio": Type.Optional(Type.Union([Type.String(), Type.Null()]))
```

---

## Type mapping

| SQL type | TypeBox type |
|----------|-------------|
| `integer`, `serial`, `bigint`, `float`, `numeric` | `Type.Number()` |
| `text`, `varchar`, `char` | `Type.String()` |
| `boolean` | `Type.Boolean()` |
| `timestamp`, `timestamptz`, `date` | `Type.Date()` |
| `json`, `jsonb` | `Type.Any()` |
| `uuid` | `Type.String()` |
| `bytea` | `Type.Uint8Array()` |
| `text[]`, `integer[]` | `Type.Array(...)` |
| enum type | `Type.Union([Type.Literal(...), ...])` |

---

## Overrides

Use `overrides` to change how a specific SQL type maps to a TypeBox type:

```toml
[[targets]]
language = "typescript"
out      = "src/generated"
schema   = "sql/schema.sql"
driver   = "bun-sql"

[targets.overrides]
uuid = "string"
```

Supported override values: `"string"`, `"number"`, `"boolean"`.

With `uuid = "string"`, any `uuid` column generates `Type.String()` instead of the default behavior.

---

## Using the generated types

```typescript
import { SelectUsers, InsertUsers } from './generated/schema';
import type { Static } from '@sinclair/typebox';

// Types are inferred from the validators — no duplication
type User = Static<typeof SelectUsers>;
//   ^? { id: number; name: string; bio: string | null; status: "active" | "inactive" | "banned"; ... }

// Runtime validation
const parsed = SelectUsers.safeParse(rawDbRow);
```

The `Prettify<>` wrapper ensures IDE tooltips show the fully expanded object shape rather than `Static<typeof SelectUsers>`.

---

## Next steps

- [Zod](/typescript/zod) — use Zod v4 or v3 instead of TypeBox
- [Bun SQL](/typescript/bun-sql) — generate typed query functions for Bun
- [pg Driver](/typescript/pg) — generate typed query functions for node-postgres
