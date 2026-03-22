---
title: Zod
description: Generate Zod v4 or v3 schemas from your SQL tables with full type inference.
---

import { Tabs, TabItem } from '@astrojs/starlight/components';

sqlcx supports both Zod v4 and Zod v3. Both generate a `schema.ts` file with validators for every table. The schemas are structurally identical — the only differences are the import path and a small number of type-specific API changes between versions.

---

## Configuration

<Tabs>
  <TabItem label="Zod v4">
    ```toml
    [[targets]]
    language = "typescript"
    out      = "src/generated"
    schema   = "sql/schema.sql"
    driver   = "bun-sql"

    [targets.schema_options]
    schema = "zod"
    ```

    ```bash
    npm install zod  # v4+
    ```
  </TabItem>
  <TabItem label="Zod v3">
    ```toml
    [[targets]]
    language = "typescript"
    out      = "src/generated"
    schema   = "sql/schema.sql"
    driver   = "bun-sql"

    [targets.schema_options]
    schema = "zod/v3"
    ```

    ```bash
    npm install zod@^3
    ```
  </TabItem>
</Tabs>

---

## Why two versions?

Zod v4 shipped a breaking API change: `z.instanceof()` was removed in favour of `z.custom<T>()`. sqlcx supports both because many projects are still on Zod v3 and the upgrade is not always straightforward. Use `schema = "zod"` for new projects.

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

<Tabs>
  <TabItem label="Zod v4">
    sqlcx writes `schema.ts`:

    ```typescript
    import { z } from "zod";

    type Prettify<T> = { [K in keyof T]: T[K] } & {};

    export const UserStatus = z.enum(["active", "inactive", "banned"]);

    export const SelectUsers = z.object({
      "id": z.number(),
      "name": z.string(),
      "bio": z.string().nullable(),
      "status": UserStatus,
      "tags": z.array(z.string()).nullable(),
      "created_at": z.date()
    });

    export const InsertUsers = z.object({
      "id": z.number().optional(),
      "name": z.string(),
      "bio": z.string().nullable().optional(),
      "status": UserStatus.optional(),
      "tags": z.array(z.string()).nullable().optional(),
      "created_at": z.date().optional()
    });

    export type SelectUsers = Prettify<z.infer<typeof SelectUsers>>;
    export type InsertUsers = Prettify<z.infer<typeof InsertUsers>>;
    export type UserStatus = Prettify<z.infer<typeof UserStatus>>;
    ```
  </TabItem>
  <TabItem label="Zod v3">
    sqlcx writes `schema.ts`:

    ```typescript
    import { z } from "zod";

    export const UserStatus = z.enum(["active", "inactive", "banned"]);

    export const SelectUsers = z.object({
      "id": z.number(),
      "name": z.string(),
      "bio": z.string().nullable(),
      "status": UserStatus,
      "tags": z.array(z.string()).nullable(),
      "created_at": z.date()
    });

    export const InsertUsers = z.object({
      "id": z.number().optional(),
      "name": z.string(),
      "bio": z.string().nullable().optional(),
      "status": UserStatus.optional(),
      "tags": z.array(z.string()).nullable().optional(),
      "created_at": z.date().optional()
    });

    export type SelectUsers = z.infer<typeof SelectUsers>;
    export type InsertUsers = z.infer<typeof InsertUsers>;
    export type UserStatus = z.infer<typeof UserStatus>;
    ```
  </TabItem>
</Tabs>

---

## How types are derived

### Select schemas

Nullable columns get `.nullable()` chained:

```typescript
// NOT NULL column
"name": z.string()

// Nullable column
"bio": z.string().nullable()
```

### Insert schemas

Columns with a `DEFAULT` or that are nullable become `.optional()`:

```typescript
// Required, no default
"name": z.string()

// Has DEFAULT — optional on insert
"status": UserStatus.optional()

// Nullable — optional on insert
"bio": z.string().nullable().optional()
```

---

## Type mapping

| SQL type | Zod v4 | Zod v3 |
|----------|--------|--------|
| `integer`, `serial`, `bigint`, `float`, `numeric` | `z.number()` | `z.number()` |
| `text`, `varchar`, `char` | `z.string()` | `z.string()` |
| `boolean` | `z.boolean()` | `z.boolean()` |
| `timestamp`, `timestamptz`, `date` | `z.date()` | `z.date()` |
| `json`, `jsonb` | `z.unknown()` | `z.unknown()` |
| `uuid` | `z.string().uuid()` | `z.string().uuid()` |
| `bytea` | `z.custom<Uint8Array>()` | `z.instanceof(Uint8Array)` |
| `text[]`, `integer[]` | `z.array(...)` | `z.array(...)` |
| enum type | `z.enum([...])` | `z.enum([...])` |

The only difference between v4 and v3 is the `bytea` mapping. Zod v4 removed `z.instanceof()`, so sqlcx uses `z.custom<Uint8Array>()` instead.

---

## Overrides

Use `overrides` to change how a specific SQL type maps:

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

---

## Using the generated schemas

```typescript
import { SelectUsers, InsertUsers } from './generated/schema';

// Infer types
type User = z.infer<typeof SelectUsers>;
//   ^? { id: number; name: string; bio: string | null; status: "active" | "inactive" | "banned"; ... }

// Runtime parse (throws on failure)
const user = SelectUsers.parse(rawDbRow);

// Safe parse (returns { success, data, error })
const result = InsertUsers.safeParse(body);
if (!result.success) {
  console.error(result.error.issues);
}
```

---

## Next steps

- [TypeBox](/typescript/typebox) — use TypeBox instead of Zod
- [Bun SQL](/typescript/bun-sql) — generate typed query functions for Bun
- [pg Driver](/typescript/pg) — generate typed query functions for node-postgres
