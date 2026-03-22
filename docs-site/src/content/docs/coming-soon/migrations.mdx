---
title: Migrations
description: SQL migration management is coming to sqlcx.
---

:::caution[Coming Soon]
Migration support is not yet available. This page previews what is planned.
:::

## What's Planned

sqlcx will include a migration system that stays true to the SQL-first philosophy — no DSL, no JavaScript migration files. You write SQL, and sqlcx tracks what has changed.

### Core Features

- **Schema diffing** — compare two versions of `schema.sql` and produce `ALTER TABLE` statements automatically
- **Migration files** — timestamped `.sql` files for each schema change, committed alongside your code
- **Up / Down** — each migration includes a forward (`up`) and rollback (`down`) statement
- **Apply tracking** — a `_sqlcx_migrations` table tracks what has been applied in each environment

---

## How It Will Work

### Generating a migration

When you change `schema.sql`, run:

```bash
sqlcx migrate generate --name add_users_verified_column
```

sqlcx diffs the current schema against the last applied migration and generates:

```
migrations/
  0001_init.sql
  0002_add_users_verified_column.sql   ← new
```

The generated file:

```sql
-- Migration: 0002_add_users_verified_column
-- Created: 2026-03-22

-- up
ALTER TABLE users ADD COLUMN verified BOOLEAN NOT NULL DEFAULT FALSE;

-- down
ALTER TABLE users DROP COLUMN verified;
```

### Applying migrations

```bash
sqlcx migrate up          # apply all pending
sqlcx migrate up --dry-run
sqlcx migrate down --steps 1
sqlcx migrate status      # show applied vs pending
```

---

## Want This Sooner?

Open an issue on GitHub describing your migration workflow. Are you using Flyway, Liquibase, or raw SQL today? That context helps prioritize the right design.
