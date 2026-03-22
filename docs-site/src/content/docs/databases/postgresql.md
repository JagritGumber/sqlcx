---
title: PostgreSQL
description: Full PostgreSQL support in sqlcx — enums, arrays, JSONB, UUID, and more.
---

sqlcx supports PostgreSQL's rich type system, including custom `ENUM` types, array columns, `JSONB`, `UUID`, and positional `$1`/`$2` parameters.

## Configuration

```toml
[sqlcx]
parser = "postgres"
```

## Supported Features

- `CREATE TYPE … AS ENUM` (separate enum declarations)
- `CREATE TABLE` with full column definitions
- `SERIAL` / `BIGSERIAL` auto-increment shortcuts
- Array columns: `TEXT[]`, `INTEGER[]`
- Column `DEFAULT` expressions
- `NOT NULL` constraints
- Positional parameters: `$1`, `$2`, `$3`, …

## Type Mapping

| SQL Type | Generated Type |
|---|---|
| `SERIAL`, `INTEGER`, `INT`, `SMALLINT`, `BIGINT` | `integer` |
| `REAL`, `DOUBLE PRECISION`, `NUMERIC`, `DECIMAL` | `float` |
| `TEXT`, `VARCHAR`, `CHAR`, `UUID` | `string` |
| `BOOLEAN` | `boolean` |
| `TIMESTAMP`, `TIMESTAMPTZ`, `DATE`, `TIME` | `datetime` |
| `JSONB`, `JSON` | `json` |
| `BYTEA` | `binary` |
| `TEXT[]`, `INTEGER[]`, … | `array` |
| Custom `ENUM` type | `enum` |

## Example Schema

```sql
CREATE TYPE user_status AS ENUM ('active', 'inactive', 'banned');

CREATE TABLE users (
  id         SERIAL PRIMARY KEY,
  email      TEXT NOT NULL,
  status     user_status NOT NULL DEFAULT 'active',
  tags       TEXT[],
  metadata   JSONB,
  created_at TIMESTAMP NOT NULL DEFAULT now()
);
```

sqlcx will generate a `UserStatus` enum with variants `Active`, `Inactive`, and `Banned`, and a `Users` type with all columns mapped accordingly.

## PostgreSQL-Specific Notes

### ENUMs

PostgreSQL ENUMs are declared separately with `CREATE TYPE … AS ENUM (…)`. sqlcx resolves column references to their enum type automatically. The enum values are PascalCased in generated code.

### Arrays

Array columns such as `TEXT[]` are mapped to the `array` type. The element type is inferred from the base type (e.g., `TEXT[]` → array of strings).

### UUID

`UUID` columns map to `string`. If your target language has a dedicated UUID type, you can configure a custom override in `sqlcx.toml`.

### JSONB / JSON

Both `JSONB` and `JSON` map to the `json` type, which generates as an untyped object or `any` depending on the language target. Use inline type overrides for stronger typing.

### Positional Parameters

PostgreSQL uses `$1`, `$2`, … for query parameters. sqlcx parses these and generates typed parameter lists in the correct order.
