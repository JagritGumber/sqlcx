---
title: Watch Mode
description: Auto-regenerate typed code when SQL files change.
---

:::caution[Coming Soon]
Watch mode is not yet available. This page previews what is planned.
:::

## What's Planned

`sqlcx watch` will monitor your `sql/` directory and automatically regenerate type-safe code whenever a `.sql` file is saved — the same experience as `tsc --watch` for TypeScript.

---

## How It Will Work

Start the watcher in your project:

```bash
sqlcx watch
```

Output:

```
sqlcx watching sql/ → src/db/
  ✓ initial generation complete (12 queries across 3 files)
  waiting for changes...

  [12:04:31] changed: sql/users.sql
  ✓ regenerated src/db/users.ts (3 queries, 0 errors)

  [12:05:10] changed: sql/orders.sql
  ✗ parse error: sql/orders.sql:14 — unknown column "user_uuid"
```

### Features

- **Incremental** — only regenerates files affected by the changed SQL file
- **Error reporting** — parse and type errors surface immediately in the terminal
- **Editor integration** — errors are written to a `.sqlcx-errors` file that editors can pick up
- **Config-aware** — respects `sqlcx.toml` for output paths and language targets

---

## Integration with Dev Servers

Watch mode is designed to run alongside your existing dev server. In a typical setup:

```json
{
  "scripts": {
    "dev": "concurrently \"sqlcx watch\" \"vite dev\""
  }
}
```

When a SQL file changes, sqlcx regenerates the TypeScript output, which triggers a hot reload in Vite — no manual step needed.

---

## Want This Sooner?

Star the repo or open an issue. If you have a specific editor or bundler integration in mind (VS Code tasks, Vite plugin, Next.js dev server), mention it in the issue.
