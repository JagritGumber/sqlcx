# Advanced Example

Multiple tables with relations, enums, @json/@enum annotations, RETURNING clause, @param overrides.

## Run

From the repo root:

```bash
bun run src/cli/index.ts generate --sql examples/advanced/sql --out examples/advanced/src/db
```

## Features demonstrated

- Multiple tables with foreign keys
- `CREATE TYPE ... AS ENUM` for post status
- Inline `@enum()` annotation for user roles
- `@json()` annotations for typed JSONB columns
- `TEXT[]` array columns
- `RETURNING *` on INSERT
- `@param` annotation overrides for ambiguous params
- All query commands: `:one`, `:many`, `:exec`, `:execresult`
