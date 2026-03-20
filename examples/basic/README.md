# Basic Example

Simple CRUD operations on a users table.

## Run

From the repo root:

```bash
bun run src/cli/index.ts generate --sql examples/basic/sql --out examples/basic/src/db
```

## What it generates

- `src/db/schema.ts` — TypeBox schemas + TypeScript types for `users`
- `src/db/client.ts` — DatabaseClient interface + BunSqlClient adapter
- `src/db/users.queries.ts` — Typed query functions
