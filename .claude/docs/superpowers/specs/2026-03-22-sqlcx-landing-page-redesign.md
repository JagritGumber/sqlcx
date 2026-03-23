# sqlcx Landing Page Redesign — Design Spec

**Date:** 2026-03-22
**Status:** Approved

## Goal

Redesign the sqlcx docs landing page from generic Starlight default to a bold, power-focused developer tool page. Dark theme, lime green accents, zero clutter.

## Design Philosophy

- **Power/Speed** — bold type, stark contrast, confidence
- **One job per section** — each section proves one thing
- **No clutter** — the current page crams install tabs, comparison table, and code all in one scroll. Kill that.

## Icon

SVG icon (tilted database cylinder = letter S). Inline SVG, not an image file. Uses `fill="currentColor"` — set `color: #c5d96e` on the parent to render lime green.

```svg
<svg width="240" height="240" viewBox="0 0 240 240" fill="none" xmlns="http://www.w3.org/2000/svg">
<path d="M28 187.257C49.9979 198.68 83.4849 200.513 120.573 193.974C157.602 187.444 188.436 174.29 205.295 156.167L209.205 178.345C213.416 202.227 177.682 223.871 130.539 232.813L127.519 233.366C78.8889 241.941 36.2413 233.996 31.9401 209.602L28 187.257Z" fill="currentColor"/>
<path d="M34.886 89C51.6501 107.258 82.4907 120.434 119.579 126.974C156.607 133.503 190.081 131.688 212.122 120.424L208.211 142.602C203.91 166.996 161.262 174.941 112.633 166.366C65.3816 158.034 28.7621 136.975 30.7967 113.401L30.8146 112.378L30.9459 111.345L34.886 89Z" fill="currentColor"/>
<path d="M113.054 20.6077C123.306 18.8 133.302 17.728 142.716 17.3881L147.458 17.2729C153.135 17.2196 158.542 17.444 163.681 17.946L168.242 18.472L169.022 18.5781C171.857 18.9868 174.676 19.506 177.471 20.1347L179.58 20.6361C183.372 21.6055 186.853 22.7856 190.023 24.1764C191.234 24.7076 192.391 25.265 193.496 25.8487C196.294 27.3247 198.897 29.1444 201.245 31.2655L202.363 32.368C202.944 32.9763 203.49 33.5943 204 34.222L204.931 35.4693C206.406 37.5785 207.485 39.8591 208.168 42.3111L208.633 44.3717C212.934 68.7653 175.576 90.8176 126.946 99.3923C79.6948 107.724 38.0812 100.46 31.9301 77.6107C31.6747 76.9711 31.4861 76.3067 31.3673 75.6283L31.2359 74.5954L31.2199 73.5829C29.2344 50.5172 64.2284 29.8567 109.985 21.1692L113.054 20.6077Z" fill="currentColor"/>
</svg>
```

## Product Name Treatment

The icon IS the letter S. Next to it, render **"QLCX"** in **Passion One** (Google Font, Bold 700) so it reads as **SQLCX**. Layout: icon + text side-by-side, vertically centered.

- Icon: 64px tall
- "QLCX" text: ~3.5rem, Passion One Bold, white (#ffffff)
- Load Passion One via Google Fonts `<link>` in the Astro layout head, or `@import` in CSS. Only used for the product name — NOT for body text or headings.

## Color Palette

- Accent: `#c5d96e` (lime green)
- Background: `#0a0a0a` (near-black)
- Text primary: `#ffffff`
- Text muted: `#888888`
- Card bg: `#111111`
- Card border: `#222222`
- Glow: `radial-gradient` of `#c5d96e` at 8-10% opacity

## Sections

### Section 1: Hero

**Layout:** Centered single column, vertically + horizontally centered. `min-h-screen` on desktop (`md:min-h-screen`), natural height on mobile.

- **Logo lockup** — icon SVG (64px) + "QLCX" text (Passion One Bold, white) side-by-side. Subtle radial glow behind the icon (~120px blur, lime at 10% opacity).
- **Tagline** — `SQL-first. Every language. Zero runtime.` — ~3rem on mobile, ~3.5rem on desktop, font-bold, white. The word **"Zero"** in lime green (#c5d96e) for visual punch.
- **Subtitle** — "Write SQL once, generate type-safe code for TypeScript, Go, and Rust." — ~1.1rem, text-gray-400.
- **Stats pills** — three inline badges: `3 databases` · `3 languages` · `0 KB runtime` — glass-morphism style (bg-white/5, border-white/10, backdrop-blur-sm, rounded-full, px-4 py-1.5, text-sm).
- **CTAs** — "Get Started" (lime bg #c5d96e, black text, font-semibold, rounded-md, px-6 py-2.5) + "GitHub" (ghost outline, lime border + lime text).

Nothing else in the hero. No install commands, no code, no comparison.

### Section 2: SQL in → Code out

**Layout:** Two-column (`md:grid-cols-2`) side-by-side code blocks on desktop, stacked on mobile. Breakpoint: `md:` (768px). Max-width: 56rem, centered.

- Left: SQL input — schema + query in one block, syntax highlighted
- Right: Generated TypeScript output — schema types only (NOT driver functions — those are in a separate file)
- Small label above each: "Write SQL" / "Get types" — text-xs uppercase tracking-wide text-gray-500
- Code blocks: bg-[#111] border border-[#222] rounded-lg p-4 overflow-x-auto
- This is ONE example, not tabs.

**SQL input:**
```sql
CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  email TEXT NOT NULL UNIQUE,
  status user_status NOT NULL DEFAULT 'active'
);

-- name: GetUser :one
SELECT * FROM users WHERE id = $1;
```

**TypeScript output (schema.ts):**
```typescript
import { Type, type Static } from "@sinclair/typebox";

export const UsersSelect = Type.Object({
  id: Type.Number(),
  name: Type.String(),
  email: Type.String(),
  status: Type.Union([
    Type.Literal("active"),
    Type.Literal("inactive"),
    Type.Literal("banned"),
  ]),
});

export type UsersSelect = Static<typeof UsersSelect>;
```

### Section 3: Zero runtime

**Layout:** Max-width 36rem, centered. Horizontal performance bars.

- Title: "Zero runtime overhead" — text-2xl font-bold white, centered
- Three rows, each with: tool name (left), bar (middle), size label (right)
  - **sqlcx**: 0 KB — NO bar at all, just a lime green dot/circle marker (w-2 h-2 rounded-full bg-lime). This is the visual punch: nothing to show because there IS nothing.
  - **Drizzle**: 7.4 KB — narrow gray bar (~5% width relative to Prisma)
  - **Prisma**: 1.6 MB — full-width gray bar
- Bar colors: sqlcx marker = lime, others = gray-700
- Tool name: text-sm text-gray-300. sqlcx name in lime + font-semibold.
- Size labels: text-sm font-mono text-gray-500, right-aligned

### Section 4: Every language

**Layout:** Three columns (`md:grid-cols-3`) on desktop, stacked on mobile. Max-width 56rem, centered.

- Title: "Every language. Same SQL." — text-2xl font-bold white, centered
- Three cards, each with: language pill label at top-left, code snippet below
- Cards: bg-[#111] border border-[#222] rounded-lg p-4

**TypeScript card:**
```typescript
export const UsersSelect = Type.Object({
  id: Type.Number(),
  name: Type.String(),
  email: Type.String(),
  status: Type.Union([...]),
});
```

**Go card:**
```go
type UsersSelect struct {
  ID     int32   `db:"id" json:"id"`
  Name   string  `db:"name" json:"name"`
  Email  string  `db:"email" json:"email"`
  Status string  `db:"status" json:"status"`
}
```

**Rust card:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsersSelect {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub status: String,
}
```

### Section 5: Get started

**Layout:** Centered, max-width 28rem. Minimal.

- Title: "Get started in seconds" — text-2xl font-bold white, centered
- Two-tab install (npm / cargo):
  - npm: `npm install sqlcx-orm`
  - cargo: `cargo install sqlcx`
- Then: `npx sqlcx generate` — styled as a terminal command block
- Below: "Read the Quick Start guide →" link in lime

### Section 6: Footer CTA

- Subtle top border (border-t border-[#222])
- Centered text: "SQL is the source of truth." — text-xl text-gray-400
- "Get Started" button (lime) + "View on GitHub" text link
- Padding: py-16

## Implementation

All sections are Astro components in `docs-site/src/components/` using Tailwind v4 utility classes. No custom CSS except Starlight variable overrides (which must remain as CSS since Starlight reads them directly).

**Components:**
- `Hero.astro` — Section 1 (replaces existing Hero.astro)
- `CodeComparison.astro` — Section 2
- `RuntimeBars.astro` — Section 3
- `LanguageCards.astro` — Section 4
- `GetStarted.astro` — Section 5
- `FooterCTA.astro` — Section 6

**Landing page** (`index.mdx`) imports all 6 components sequentially. Each section has `py-16 md:py-24` vertical padding for breathing room between sections.

**Font loading:** Add Passion One (Bold 700 only) via Google Fonts `<link>` tag in the Astro head, or via `@import` in custom.css. Only used for the SQLCX logo lockup.

## What gets removed

- Current Hero.astro with banner.png image reference
- ComparisonTable.astro (replaced by RuntimeBars.astro)
- banner.png from src/assets/ (no longer needed in hero — still kept in public/ for OG/social)
- Install tabs with 4 package managers in the hero area
- Inline code tabs in the hero area
