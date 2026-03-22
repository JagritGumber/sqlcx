---
title: DSL Compiler
description: A compact DSL for defining queries that compiles to standard SQL.
---

:::caution[Coming Soon]
The DSL compiler is not yet available. This page previews what is planned.
:::

## What's Planned

sqlcx will support `.sqlcx` files as a compact DSL for defining queries. The compiler transforms this DSL into standard SQL before code generation runs — so the output is identical to writing SQL by hand.

The DSL is not a replacement for SQL. It is a shorthand for common patterns that are verbose to write repeatedly.

---

## What It Looks Like

A `.sqlcx` file:

```
query get_user_by_id :one {
  from users
  where id = :id
  select id, email, created_at
}

query list_active_users :many {
  from users
  where active = true
  order created_at desc
  limit :limit offset :offset
  select id, email
}

query create_user :exec {
  insert users {
    email = :email
    password_hash = :password_hash
  }
}
```

Compiles to standard SQL (which then goes through the normal sqlcx pipeline):

```sql
-- name: get_user_by_id :one
SELECT id, email, created_at FROM users WHERE id = $1;

-- name: list_active_users :many
SELECT id, email FROM users
WHERE active = true
ORDER BY created_at DESC
LIMIT $1 OFFSET $2;

-- name: create_user :exec
INSERT INTO users (email, password_hash) VALUES ($1, $2);
```

---

## Design Principles

- **Compiles to SQL** — the DSL is syntactic sugar, not a new query language
- **No runtime dependency** — compilation happens at codegen time, not at query execution
- **Escape hatch** — any query too complex for the DSL can be written in raw SQL and mixed freely
- **Readable output** — the compiled SQL is formatted and readable, not minified

---

## Want This Sooner?

Open an issue on GitHub. If you have a DSL syntax preference or a use case the planned syntax doesn't cover, share it — the design is still open.
