# sqlcx Documentation Site — Design Spec

**Date:** 2026-03-22
**Status:** Approved

## Goal

Build a Starlight (Astro) documentation site for sqlcx with custom branding, dark mode by default, and comprehensive guides for all supported databases, languages, and plugins.

## Framework

**Starlight** (Astro-based) — chosen over VitePress because:
- Full layout control per page (VitePress was too rigid)
- Component overrides for custom hero/sidebar
- Framework-agnostic (can use any component library)
- Tailwind integration built-in
- Pagefind search out of the box

## Branding

**Assets** at `/mnt/d/builds/sqlcx/`:
- `icon.png` — tilted database cylinder that doubles as the letter S
- `banner.png` — icon + "QLCX" text (icon IS the S, reads as SQLCX)

**Color palette:**
- Primary/accent: lime green (~#c5d96e)
- Text/logo: black (#000000)
- Dark mode: near-black background + lime green accents
- Light mode: white background + lime green accent, black text
- Dark mode is the default

**Design inspiration:** ElysiaJS docs — dark, clean, playful, developer-friendly

## Target Audiences

1. **TypeScript developers** migrating from Prisma/Drizzle — need to understand why SQL-first is better
2. **Polyglot teams** — using multiple languages on the same DB, want one tool
3. **Python developers** — upcoming target, need to know it's coming
4. **sqlc users** — already know SQL-first codegen, want cross-language

## Site Structure

### Landing Page (Hero)

- Dark background, lime green accent
- Icon/banner prominently displayed
- Tagline: **"SQL-first. Every language. Zero runtime."**
- Subtitle: "Write SQL once, generate type-safe code for TypeScript, Go, Rust, and Python. No ORM. No bloat."
- Three stats: **3 databases** · **3 languages** · **0 KB runtime**
- Multi-package-manager install tabs: npm / cargo / pip / brew
- Quick comparison card: sqlcx (0KB) vs Prisma (1.6MB) vs Drizzle (7.4KB)
- Code example: SQL input → generated TypeScript output side-by-side

### Sidebar Navigation

```
Getting Started
├── Why sqlcx?
├── Installation
├── Quick Start
└── Configuration

Databases
├── PostgreSQL
├── MySQL
└── SQLite

TypeScript
├── TypeBox
├── Zod
├── Bun.sql Driver
└── pg Driver

Go
├── Structs
└── database/sql Driver

Rust
├── Serde Structs
└── sqlx Driver

SQL Guide
├── Query Annotations
├── @enum / @json / @param
├── Input/Output Types
└── SELECT * & Partial Select

CLI Reference
├── generate
├── check
├── init
└── schema

Advanced
├── IR Format
├── Caching
├── Plugin System
└── Community Plugins

Coming Soon
├── Python (Pydantic + asyncpg)
├── Migrations
├── Watch Mode
└── DSL Compiler (.sqlcx files)

Comparison
└── sqlcx vs Prisma vs Drizzle vs sqlc
```

### Key Pages

#### Why sqlcx?
- Pain points: Prisma bloat (1.6MB), Drizzle type inference lag, sqlc Go-only
- sqlcx value prop: SQL is truth, zero runtime, multi-language, open validation
- "If you know SQL, you already know sqlcx"

#### Installation
- npm: `npm install sqlcx-orm`
- cargo: `cargo install sqlcx`
- pip: `pip install sqlcx` (coming soon)
- brew: `brew install sqlcx` (coming soon)
- Standalone binary download

#### Quick Start
- Step 1: `sqlcx init` — scaffold config + sql/ directory
- Step 2: Write schema SQL + query SQL
- Step 3: `sqlcx generate` — see generated code
- Step 4: Use generated code in your project
- Full working example from SQL → generated → usage

#### Configuration Reference
- TOML example (primary)
- JSON example (with JSON Schema)
- All config fields documented
- Multi-target configuration example
- Overrides section

#### Database Pages
Each database page covers:
- Supported SQL features
- Type mapping table (SQL type → category)
- Example schema SQL
- Database-specific quirks (e.g., MySQL inline ENUM, SQLite type affinity)

#### Language Pages
Each language section shows:
- Complete SQL → generated code example
- Schema generator options
- Driver options
- Type mapping table (category → language type)
- Usage example (import and call generated functions)

#### SQL Guide
- Query annotation format (`-- name: GetUser :one`)
- Command types (`:one`, `:many`, `:exec`, `:execresult`)
- `@enum` annotation with examples
- `@json` annotation with nested shapes
- `@param` overrides
- Input/Output type separation (Select vs Insert)
- `SELECT *` expansion and partial selects

#### CLI Reference
- `sqlcx generate` — full options
- `sqlcx check` — CI usage
- `sqlcx init` — scaffolding
- `sqlcx schema` — JSON Schema output

#### IR Format (Advanced)
- JSON structure documentation
- How caching works (SHA-256, `.sqlcx/ir.json`)
- Using IR for community plugins

#### Plugin System (Advanced)
- Trait architecture: DatabaseParser, LanguagePlugin, SchemaGenerator, DriverGenerator
- IR JSON bridge for external tools
- How to read `.sqlcx/ir.json` and generate custom code
- Process-based plugin protocol (future)

#### Comparison Page
- Feature matrix: sqlcx vs Prisma vs Drizzle vs sqlc
- Bundle size comparison
- Language support comparison
- Philosophy differences (SQL-first vs schema-first vs query builder)

## Project Structure

```
docs/                              # Starlight docs site
├── astro.config.mjs               # Starlight + Tailwind config
├── package.json
├── tailwind.config.mjs
├── tsconfig.json
├── public/
│   ├── icon.png                   # Favicon / logo
│   ├── banner.png                 # OG image
│   └── fonts/                     # Custom fonts if needed
├── src/
│   ├── assets/
│   │   └── logo.png               # Logo for sidebar
│   ├── content/
│   │   └── docs/
│   │       ├── index.mdx          # Landing page / hero
│   │       ├── getting-started/
│   │       │   ├── why-sqlcx.mdx
│   │       │   ├── installation.mdx
│   │       │   ├── quick-start.mdx
│   │       │   └── configuration.mdx
│   │       ├── databases/
│   │       │   ├── postgresql.mdx
│   │       │   ├── mysql.mdx
│   │       │   └── sqlite.mdx
│   │       ├── typescript/
│   │       │   ├── typebox.mdx
│   │       │   ├── zod.mdx
│   │       │   ├── bun-sql.mdx
│   │       │   └── pg.mdx
│   │       ├── go/
│   │       │   ├── structs.mdx
│   │       │   └── database-sql.mdx
│   │       ├── rust/
│   │       │   ├── serde.mdx
│   │       │   └── sqlx.mdx
│   │       ├── sql-guide/
│   │       │   ├── query-annotations.mdx
│   │       │   ├── annotations.mdx
│   │       │   ├── input-output-types.mdx
│   │       │   └── select-patterns.mdx
│   │       ├── cli/
│   │       │   ├── generate.mdx
│   │       │   ├── check.mdx
│   │       │   ├── init.mdx
│   │       │   └── schema.mdx
│   │       ├── advanced/
│   │       │   ├── ir-format.mdx
│   │       │   ├── caching.mdx
│   │       │   ├── plugin-system.mdx
│   │       │   └── community-plugins.mdx
│   │       ├── coming-soon/
│   │       │   ├── python.mdx
│   │       │   ├── migrations.mdx
│   │       │   ├── watch-mode.mdx
│   │       │   └── dsl-compiler.mdx
│   │       └── comparison.mdx
│   ├── components/
│   │   ├── Hero.astro             # Custom hero with banner
│   │   └── ComparisonTable.astro  # Bundle size comparison
│   └── styles/
│       └── custom.css             # Lime green theme overrides
```

## Theming

### Starlight CSS Variables

```css
:root {
  --sl-color-accent-low: #2a3000;
  --sl-color-accent: #c5d96e;
  --sl-color-accent-high: #e8f0a0;
  --sl-color-text-accent: #c5d96e;
  --sl-font-system: 'Inter', system-ui, sans-serif;
  --sl-font-system-mono: 'JetBrains Mono', monospace;
}
```

### Dark Mode (default)

```css
:root[data-theme='dark'] {
  --sl-color-bg: #0a0a0a;
  --sl-color-bg-sidebar: #111111;
}
```

## Hosting

- Default: `sqlcx.jagritgumber.com` (subdomain)
- Future: dedicated domain if purchased
- Platform: Vercel (free tier, automatic deploys from GitHub)
- Build: `astro build` → static output

## Content Guidelines

- Every code example should be copy-pasteable
- Show SQL input → generated output on every language/schema page
- Use Starlight's built-in `<Tabs>` component for multi-language examples
- Keep pages focused — one topic per page, link to related pages
- "Coming Soon" pages should briefly describe what's planned with estimated timeline
