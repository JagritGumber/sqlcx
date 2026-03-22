# sqlcx Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan. Each task runs in an isolated git worktree branch and merges to main when complete. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a working `sqlcx generate` CLI that reads PostgreSQL schema + annotated queries and outputs type-safe TypeScript with TypeBox v1.0 schemas and Bun.sql driver adapters.

**Architecture:** Bottom-up build: IR types → parser → generators → CLI orchestration. Each layer is independently testable. The IR is the stable interface between parsing and generation.

**Tech Stack:** Bun runtime, TypeScript, node-sql-parser, Bun test runner

**Spec:** `docs/superpowers/specs/2026-03-18-sqlcx-design.md`

---

## Branch Strategy

Each task runs in its own worktree branch off `main`. Tasks are **sequential** — each branch is created from the merged state of the previous task. After each task's subagent completes, merge to `main` before starting the next.

```
main ←── feat/01-scaffolding
     ←── feat/02-ir-types
     ←── feat/03-param-naming
     ←── feat/04-postgres-parser
     ←── feat/05-generator-interfaces
     ←── feat/06-typebox-generator     (can parallel with 07)
     ←── feat/07-bun-sql-driver        (can parallel with 06)
     ←── feat/08-ts-plugin
     ←── feat/09-config
     ←── feat/10-cache
     ←── feat/11-cli
     ←── feat/12-e2e-tests
     ←── feat/13-package-finalization
```

**Parallelizable tasks:** Tasks 6 and 7 can run in parallel (both depend on Task 5, neither depends on each other). All other tasks are sequential.

**Branch naming:** `feat/NN-short-name` (e.g., `feat/01-scaffolding`)

**Merge protocol:** After each task completes and tests pass:
1. Merge branch to `main` (fast-forward or merge commit)
2. Delete the branch
3. Push `main` to origin

---

## File Map

```
sqlcx/
├── src/
│   ├── ir/
│   │   └── index.ts              # IR type definitions (TableDef, ColumnDef, SqlType, etc.)
│   ├── parser/
│   │   ├── interface.ts          # DatabaseParser interface
│   │   ├── postgres.ts           # PostgreSQL parser implementation
│   │   └── param-naming.ts       # Parameter name inference logic
│   ├── generator/
│   │   ├── interface.ts          # LanguagePlugin, SchemaGenerator, DriverGenerator interfaces
│   │   └── typescript/
│   │       ├── index.ts          # TypeScript language plugin (orchestrates schema + driver)
│   │       ├── schema/
│   │       │   └── typebox.ts    # TypeBox v1.0 schema generator
│   │       └── driver/
│   │           └── bun-sql.ts    # Bun.sql driver generator
│   ├── config/
│   │   └── index.ts              # defineConfig + config loading from sqlcx.config.ts
│   ├── cache/
│   │   └── index.ts              # IR caching (SHA-256 hash, .sqlcx/ir.json)
│   ├── cli/
│   │   └── index.ts              # CLI entry point (generate, init, check)
│   └── utils/
│       └── index.ts              # Prettify type, string helpers (camelCase, PascalCase)
├── tests/
│   ├── utils/
│   │   └── utils.test.ts
│   ├── ir/
│   │   └── types.test.ts
│   ├── parser/
│   │   ├── postgres.test.ts
│   │   └── param-naming.test.ts
│   ├── generator/
│   │   └── typescript/
│   │       ├── typebox.test.ts
│   │       ├── bun-sql.test.ts
│   │       └── plugin.test.ts
│   ├── cache/
│   │   └── cache.test.ts
│   ├── cli/
│   │   └── cli.test.ts
│   └── fixtures/
│       ├── schema.sql
│       ├── queries/
│       │   └── users.sql
│       └── expected/             # Expected generated output for snapshot testing
│           ├── schema.ts
│           ├── users.queries.ts
│           └── client.ts
├── package.json
├── tsconfig.json
└── bunfig.toml
```

---

### Task 1: Project Scaffolding

**Branch:** `feat/01-scaffolding` (from `main`)
**Depends on:** nothing
**Files:**
- Create: `package.json`
- Create: `tsconfig.json`
- Create: `bunfig.toml`
- Create: `src/utils/index.ts`

- [ ] **Step 1: Initialize Bun project**

```bash
cd /home/jagrit/sqlcts
bun init -y
```

- [ ] **Step 2: Install dependencies**

```bash
bun add node-sql-parser
bun add -d @types/bun typescript
```

- [ ] **Step 3: Configure tsconfig.json**

Write `tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ESNext",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "outDir": "dist",
    "rootDir": ".",
    "declaration": true,
    "declarationMap": true,
    "sourceMap": true,
    "types": ["bun-types"],
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"]
    }
  },
  "include": ["src/**/*.ts", "tests/**/*.ts"],
  "exclude": ["node_modules", "dist"]
}
```

- [ ] **Step 4: Write utility functions**

Write `src/utils/index.ts`:
```ts
export type Prettify<T> = { [K in keyof T]: T[K] } & {};

export function pascalCase(str: string): string {
  return str
    .split(/[_\-\s]+/)
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1).toLowerCase())
    .join("");
}

export function camelCase(str: string): string {
  const pascal = pascalCase(str);
  return pascal.charAt(0).toLowerCase() + pascal.slice(1);
}
```

- [ ] **Step 5: Write utils test**

Write `tests/utils/utils.test.ts`:
```ts
import { describe, expect, test } from "bun:test";
import { pascalCase, camelCase } from "@/utils";

describe("pascalCase", () => {
  test("snake_case", () => expect(pascalCase("user_profile")).toBe("UserProfile"));
  test("kebab-case", () => expect(pascalCase("user-profile")).toBe("UserProfile"));
  test("single word", () => expect(pascalCase("users")).toBe("Users"));
});

describe("camelCase", () => {
  test("snake_case", () => expect(camelCase("user_profile")).toBe("userProfile"));
  test("single word", () => expect(camelCase("users")).toBe("users"));
});
```

- [ ] **Step 6: Run tests**

```bash
bun test tests/utils/utils.test.ts
```
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add package.json tsconfig.json bun.lockb src/utils/index.ts tests/utils/utils.test.ts .gitignore
git commit -m "feat: project scaffolding with Bun + utility functions"
```

---

### Task 2: IR Type Definitions

**Branch:** `feat/02-ir-types` (from `main` after Task 1 merged)
**Depends on:** Task 1
**Files:**
- Create: `src/ir/index.ts`
- Create: `tests/ir/types.test.ts`

- [ ] **Step 1: Write IR types**

Write `src/ir/index.ts` with all interfaces from the spec:
```ts
export type SqlTypeCategory =
  | "string"
  | "number"
  | "boolean"
  | "date"
  | "json"
  | "uuid"
  | "binary"
  | "enum"
  | "unknown";

export interface SqlType {
  raw: string;
  normalized: string;
  category: SqlTypeCategory;
  elementType?: SqlType;
  enumName?: string;
}

export interface ColumnDef {
  name: string;
  alias?: string;
  sourceTable?: string;
  type: SqlType;
  nullable: boolean;
  hasDefault: boolean;
}

export interface TableDef {
  name: string;
  columns: ColumnDef[];
  primaryKey: string[];
  uniqueConstraints: string[][];
}

export type QueryCommand = "one" | "many" | "exec" | "execresult";

export interface ParamDef {
  index: number;
  name: string;
  type: SqlType;
}

export interface QueryDef {
  name: string;
  command: QueryCommand;
  sql: string;
  params: ParamDef[];
  returns: ColumnDef[];
  sourceFile: string;
}

export interface EnumDef {
  name: string;
  values: string[];
}

export interface SqlcxIR {
  tables: TableDef[];
  queries: QueryDef[];
  enums: EnumDef[];
}
```

- [ ] **Step 2: Write IR serialization test**

Write `tests/ir/types.test.ts`:
```ts
import { describe, expect, test } from "bun:test";
import type { SqlcxIR, TableDef, SqlType } from "@/ir";

describe("IR types", () => {
  test("SqlcxIR is JSON-serializable", () => {
    const ir: SqlcxIR = {
      tables: [
        {
          name: "users",
          columns: [
            {
              name: "id",
              type: { raw: "SERIAL", normalized: "serial", category: "number" },
              nullable: false,
              hasDefault: true,
            },
            {
              name: "name",
              type: { raw: "TEXT", normalized: "text", category: "string" },
              nullable: false,
              hasDefault: false,
            },
          ],
          primaryKey: ["id"],
          uniqueConstraints: [],
        },
      ],
      queries: [
        {
          name: "GetUser",
          command: "one",
          sql: "SELECT * FROM users WHERE id = $1",
          params: [
            {
              index: 1,
              name: "id",
              type: { raw: "SERIAL", normalized: "serial", category: "number" },
            },
          ],
          returns: [
            {
              name: "id",
              type: { raw: "SERIAL", normalized: "serial", category: "number" },
              nullable: false,
              hasDefault: true,
            },
            {
              name: "name",
              type: { raw: "TEXT", normalized: "text", category: "string" },
              nullable: false,
              hasDefault: false,
            },
          ],
          sourceFile: "queries/users.sql",
        },
      ],
      enums: [],
    };

    const json = JSON.stringify(ir);
    const parsed = JSON.parse(json) as SqlcxIR;
    expect(parsed.tables).toHaveLength(1);
    expect(parsed.tables[0].name).toBe("users");
    expect(parsed.queries).toHaveLength(1);
    expect(parsed.queries[0].name).toBe("GetUser");
  });

  test("SqlType with elementType for arrays", () => {
    const arrayType: SqlType = {
      raw: "TEXT[]",
      normalized: "text[]",
      category: "string",
      elementType: { raw: "TEXT", normalized: "text", category: "string" },
    };
    const json = JSON.stringify(arrayType);
    const parsed = JSON.parse(json) as SqlType;
    expect(parsed.elementType?.category).toBe("string");
  });

  test("SqlType with enumName", () => {
    const enumType: SqlType = {
      raw: "user_status",
      normalized: "user_status",
      category: "enum",
      enumName: "user_status",
    };
    expect(enumType.enumName).toBe("user_status");
  });
});
```

- [ ] **Step 3: Run tests**

```bash
bun test tests/ir/types.test.ts
```
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/ir/index.ts tests/ir/types.test.ts
git commit -m "feat: define IR type system (TableDef, QueryDef, SqlType, etc.)"
```

---

### Task 3: Parameter Name Inference

**Branch:** `feat/03-param-naming` (from `main` after Task 2 merged)
**Depends on:** Task 2
**Files:**
- Create: `src/parser/param-naming.ts`
- Create: `tests/parser/param-naming.test.ts`

- [ ] **Step 1: Write param naming tests**

Write `tests/parser/param-naming.test.ts`:
```ts
import { describe, expect, test } from "bun:test";
import { resolveParamNames } from "@/parser/param-naming";

describe("resolveParamNames", () => {
  test("simple unique columns", () => {
    const result = resolveParamNames([
      { index: 1, column: "id" },
      { index: 2, column: "name" },
    ]);
    expect(result).toEqual(["id", "name"]);
  });

  test("collision renames both to _1 and _2", () => {
    const result = resolveParamNames([
      { index: 1, column: "created_at" },
      { index: 2, column: "created_at" },
    ]);
    expect(result).toEqual(["created_at_1", "created_at_2"]);
  });

  test("null column falls back to param_N", () => {
    const result = resolveParamNames([
      { index: 1, column: null },
    ]);
    expect(result).toEqual(["param_1"]);
  });

  test("annotation override takes precedence", () => {
    const result = resolveParamNames([
      { index: 1, column: "created_at", override: "start_date" },
      { index: 2, column: "created_at", override: "end_date" },
    ]);
    expect(result).toEqual(["start_date", "end_date"]);
  });

  test("expression extraction — column from LOWER(name)", () => {
    const result = resolveParamNames([
      { index: 1, column: "name" },
    ]);
    expect(result).toEqual(["name"]);
  });

  test("mixed: some overrides, some inferred, some collisions", () => {
    const result = resolveParamNames([
      { index: 1, column: "status" },
      { index: 2, column: "status" },
      { index: 3, column: null, override: "limit" },
    ]);
    expect(result).toEqual(["status_1", "status_2", "limit"]);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
bun test tests/parser/param-naming.test.ts
```
Expected: FAIL — module not found

- [ ] **Step 3: Implement param naming**

Write `src/parser/param-naming.ts`:
```ts
interface RawParam {
  index: number;
  column: string | null;
  override?: string;
}

export function resolveParamNames(params: RawParam[]): string[] {
  // First pass: apply overrides and collect column frequency
  const freq = new Map<string, number>();
  for (const p of params) {
    if (!p.override && p.column) {
      freq.set(p.column, (freq.get(p.column) ?? 0) + 1);
    }
  }

  // Second pass: assign names with collision suffixes
  const counters = new Map<string, number>();
  return params.map((p) => {
    if (p.override) return p.override;
    if (!p.column) return `param_${p.index}`;

    const count = freq.get(p.column) ?? 0;
    if (count > 1) {
      const n = (counters.get(p.column) ?? 0) + 1;
      counters.set(p.column, n);
      return `${p.column}_${n}`;
    }

    return p.column;
  });
}
```

- [ ] **Step 4: Run tests**

```bash
bun test tests/parser/param-naming.test.ts
```
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/parser/param-naming.ts tests/parser/param-naming.test.ts
git commit -m "feat: parameter name inference with collision handling"
```

---

### Task 4: Database Parser Interface + PostgreSQL Parser

**Branch:** `feat/04-postgres-parser` (from `main` after Task 3 merged)
**Depends on:** Tasks 2, 3
**Files:**
- Create: `src/parser/interface.ts`
- Create: `src/parser/postgres.ts`
- Create: `tests/parser/postgres.test.ts`
- Create: `tests/fixtures/schema.sql`
- Create: `tests/fixtures/queries/users.sql`

- [ ] **Step 1: Write parser interface**

Write `src/parser/interface.ts`:
```ts
import type { TableDef, QueryDef, EnumDef } from "@/ir";

export interface DatabaseParser {
  dialect: string;
  parseSchema(sql: string): TableDef[];
  parseQueries(sql: string, tables: TableDef[]): QueryDef[];
  parseEnums(sql: string): EnumDef[];
}
```

- [ ] **Step 2: Write test fixtures**

Write `tests/fixtures/schema.sql`:
```sql
CREATE TYPE user_status AS ENUM ('active', 'inactive', 'banned');

CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  email TEXT NOT NULL UNIQUE,
  bio TEXT,
  status user_status NOT NULL DEFAULT 'active',
  tags TEXT[],
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE posts (
  id SERIAL PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id),
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  published BOOLEAN NOT NULL DEFAULT FALSE,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
```

Write `tests/fixtures/queries/users.sql`:
```sql
-- name: GetUser :one
SELECT * FROM users WHERE id = $1;

-- name: ListUsers :many
SELECT id, name, email FROM users WHERE name ILIKE $1;

-- name: CreateUser :exec
INSERT INTO users (name, email, bio) VALUES ($1, $2, $3);

-- name: DeleteUser :execresult
DELETE FROM users WHERE id = $1;

-- name: ListUsersByDateRange :many
-- @param $1 start_date
-- @param $2 end_date
SELECT * FROM users WHERE created_at > $1 AND created_at < $2;
```

- [ ] **Step 3: Write parser tests**

Write `tests/parser/postgres.test.ts`:
```ts
import { describe, expect, test } from "bun:test";
import { createPostgresParser } from "@/parser/postgres";
import { readFileSync } from "fs";
import { join } from "path";

const fixturesDir = join(import.meta.dir, "../fixtures");
const schemaSql = readFileSync(join(fixturesDir, "schema.sql"), "utf-8");
const queriesSql = readFileSync(join(fixturesDir, "queries/users.sql"), "utf-8");

describe("PostgreSQL Parser", () => {
  const parser = createPostgresParser();

  describe("parseSchema", () => {
    test("parses table names", () => {
      const tables = parser.parseSchema(schemaSql);
      const names = tables.map((t) => t.name);
      expect(names).toContain("users");
      expect(names).toContain("posts");
    });

    test("parses columns with types", () => {
      const tables = parser.parseSchema(schemaSql);
      const users = tables.find((t) => t.name === "users")!;
      const idCol = users.columns.find((c) => c.name === "id")!;
      expect(idCol.type.category).toBe("number");
      expect(idCol.hasDefault).toBe(true);
    });

    test("detects nullable columns", () => {
      const tables = parser.parseSchema(schemaSql);
      const users = tables.find((t) => t.name === "users")!;
      const bioCol = users.columns.find((c) => c.name === "bio")!;
      expect(bioCol.nullable).toBe(true);
      const nameCol = users.columns.find((c) => c.name === "name")!;
      expect(nameCol.nullable).toBe(false);
    });

    test("parses primary keys", () => {
      const tables = parser.parseSchema(schemaSql);
      const users = tables.find((t) => t.name === "users")!;
      expect(users.primaryKey).toEqual(["id"]);
    });

    test("detects array columns", () => {
      const tables = parser.parseSchema(schemaSql);
      const users = tables.find((t) => t.name === "users")!;
      const tagsCol = users.columns.find((c) => c.name === "tags")!;
      expect(tagsCol.type.elementType).toBeDefined();
      expect(tagsCol.type.elementType?.category).toBe("string");
    });
  });

  describe("parseEnums", () => {
    test("parses enum types", () => {
      const enums = parser.parseEnums(schemaSql);
      expect(enums).toHaveLength(1);
      expect(enums[0].name).toBe("user_status");
      expect(enums[0].values).toEqual(["active", "inactive", "banned"]);
    });
  });

  describe("parseQueries", () => {
    test("parses query annotations", () => {
      const tables = parser.parseSchema(schemaSql);
      const queries = parser.parseQueries(queriesSql, tables);
      expect(queries).toHaveLength(4);
    });

    test("parses :one command", () => {
      const tables = parser.parseSchema(schemaSql);
      const queries = parser.parseQueries(queriesSql, tables);
      const getUser = queries.find((q) => q.name === "GetUser")!;
      expect(getUser.command).toBe("one");
      expect(getUser.params).toHaveLength(1);
      expect(getUser.params[0].name).toBe("id");
    });

    test("expands SELECT * using schema", () => {
      const tables = parser.parseSchema(schemaSql);
      const queries = parser.parseQueries(queriesSql, tables);
      const getUser = queries.find((q) => q.name === "GetUser")!;
      expect(getUser.returns.length).toBeGreaterThanOrEqual(5);
      const colNames = getUser.returns.map((c) => c.name);
      expect(colNames).toContain("id");
      expect(colNames).toContain("name");
      expect(colNames).toContain("email");
    });

    test("parses explicit column list", () => {
      const tables = parser.parseSchema(schemaSql);
      const queries = parser.parseQueries(queriesSql, tables);
      const listUsers = queries.find((q) => q.name === "ListUsers")!;
      expect(listUsers.returns).toHaveLength(3);
      expect(listUsers.returns.map((c) => c.name)).toEqual(["id", "name", "email"]);
    });

    test("parses :exec with no returns", () => {
      const tables = parser.parseSchema(schemaSql);
      const queries = parser.parseQueries(queriesSql, tables);
      const createUser = queries.find((q) => q.name === "CreateUser")!;
      expect(createUser.command).toBe("exec");
      expect(createUser.returns).toHaveLength(0);
    });

    test("parses :execresult", () => {
      const tables = parser.parseSchema(schemaSql);
      const queries = parser.parseQueries(queriesSql, tables);
      const deleteUser = queries.find((q) => q.name === "DeleteUser")!;
      expect(deleteUser.command).toBe("execresult");
    });

    test("parses @param annotation overrides", () => {
      const tables = parser.parseSchema(schemaSql);
      const queries = parser.parseQueries(queriesSql, tables);
      const dateRange = queries.find((q) => q.name === "ListUsersByDateRange")!;
      expect(dateRange.params).toHaveLength(2);
      expect(dateRange.params[0].name).toBe("start_date");
      expect(dateRange.params[1].name).toBe("end_date");
    });
  });
});
```

- [ ] **Step 4: Run tests to verify they fail**

```bash
bun test tests/parser/postgres.test.ts
```
Expected: FAIL — module not found

- [ ] **Step 5: Implement PostgreSQL parser**

Write `src/parser/postgres.ts`. This is the largest single file. It must:
1. Use `node-sql-parser` with PostgreSQL dialect to parse CREATE TABLE statements
2. Extract column names, types, nullable, defaults, primary keys, unique constraints
3. Parse `CREATE TYPE ... AS ENUM` for enum extraction
4. Parse query annotations (`-- name: X :command`) and split SQL into individual queries
5. For each query, extract `$N` params with type inference from the schema
6. For SELECT queries, resolve return columns (expand `*` using schema, or use explicit list)
7. Use `param-naming.ts` for parameter name inference

Key implementation details:
- `node-sql-parser` may not handle all Postgres syntax perfectly. For enum parsing and annotation parsing, use regex-based extraction as fallback.
- SQL type → category mapping: maintain a `Map<string, SqlTypeCategory>` for common Postgres types (text→string, integer→number, boolean→boolean, timestamp→date, jsonb→json, uuid→uuid, bytea→binary, etc.)
- Array types: detect `TYPE[]` pattern, set `elementType`
- Enum types: cross-reference column type name against parsed enums, set `category: "enum"` and `enumName`

- [ ] **Step 6: Run tests**

```bash
bun test tests/parser/postgres.test.ts
```
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src/parser/ tests/parser/ tests/fixtures/
git commit -m "feat: PostgreSQL parser with schema, query, and enum extraction"
```

---

### Task 5: Generator Interfaces

**Branch:** `feat/05-generator-interfaces` (from `main` after Task 4 merged)
**Depends on:** Task 2
**Files:**
- Create: `src/generator/interface.ts`

- [ ] **Step 1: Write all generator interfaces**

Write `src/generator/interface.ts`:
```ts
import type { SqlcxIR, TableDef, QueryDef, EnumDef, SqlType } from "@/ir";

export interface LanguageOptions {
  out: string;
  overrides?: Record<string, string>;
}

export interface GeneratedFile {
  path: string;
  content: string;
}

export interface SchemaGenerator {
  name: string;
  generateImports(): string;
  generateEnumSchema(enumDef: EnumDef): string;
  generateSelectSchema(table: TableDef, ir: SqlcxIR): string;
  generateInsertSchema(table: TableDef, ir: SqlcxIR): string;
  generateTypeAlias(name: string, schemaVarName: string): string;
}

export interface DriverGenerator {
  name: string;
  generateImports(): string;
  generateClientAdapter(): string;
  generateQueryFunction(query: QueryDef): string;
}

// NOTE: DatabaseClient interface is generated by the LanguagePlugin (TypeScript
// plugin in src/generator/typescript/index.ts), NOT by each DriverGenerator.
// This prevents collisions when multiple drivers target the same output dir.

export interface LanguagePlugin {
  language: string;
  fileExtension: string;
  generate(ir: SqlcxIR, options: LanguageOptions): GeneratedFile[];
}
```

- [ ] **Step 2: Commit**

```bash
git add src/generator/interface.ts
git commit -m "feat: define generator plugin interfaces (Language, Schema, Driver)"
```

---

### Task 6: TypeBox Schema Generator

**Branch:** `feat/06-typebox-generator` (from `main` after Task 5 merged)
**Depends on:** Task 5 | **Can parallel with:** Task 7
**Files:**
- Create: `src/generator/typescript/schema/typebox.ts`
- Create: `tests/generator/typescript/typebox.test.ts`

- [ ] **Step 1: Write TypeBox generator tests**

Write `tests/generator/typescript/typebox.test.ts`:
```ts
import { describe, expect, test } from "bun:test";
import { createTypeBoxGenerator } from "@/generator/typescript/schema/typebox";
import type { SqlcxIR, TableDef, EnumDef } from "@/ir";

const generator = createTypeBoxGenerator();

const usersTable: TableDef = {
  name: "users",
  columns: [
    { name: "id", type: { raw: "SERIAL", normalized: "serial", category: "number" }, nullable: false, hasDefault: true },
    { name: "name", type: { raw: "TEXT", normalized: "text", category: "string" }, nullable: false, hasDefault: false },
    { name: "email", type: { raw: "TEXT", normalized: "text", category: "string" }, nullable: false, hasDefault: false },
    { name: "bio", type: { raw: "TEXT", normalized: "text", category: "string" }, nullable: true, hasDefault: false },
    { name: "created_at", type: { raw: "TIMESTAMP", normalized: "timestamp", category: "date" }, nullable: false, hasDefault: true },
  ],
  primaryKey: ["id"],
  uniqueConstraints: [["email"]],
};

const ir: SqlcxIR = { tables: [usersTable], queries: [], enums: [] };

describe("TypeBox Schema Generator", () => {
  test("generates imports", () => {
    const imports = generator.generateImports();
    expect(imports).toContain("typebox");
    expect(imports).toContain("Type");
    expect(imports).toContain("Static");
  });

  test("generates SelectUser schema with all columns", () => {
    const schema = generator.generateSelectSchema(usersTable, ir);
    expect(schema).toContain("SelectUser");
    expect(schema).toContain("Type.Number()");  // id
    expect(schema).toContain("Type.String()");  // name, email
    expect(schema).toContain("Type.Null()");    // bio is nullable
    expect(schema).toContain("Type.Date()");    // created_at
  });

  test("generates InsertUser schema omitting columns with defaults", () => {
    const schema = generator.generateInsertSchema(usersTable, ir);
    expect(schema).toContain("InsertUser");
    expect(schema).not.toContain('"id"');        // has default
    expect(schema).not.toContain('"created_at"'); // has default
    expect(schema).toContain('"name"');
    expect(schema).toContain('"email"');
  });

  test("nullable columns use Union with Null in Select", () => {
    const schema = generator.generateSelectSchema(usersTable, ir);
    // bio should be Type.Union([Type.String(), Type.Null()])
    expect(schema).toContain("Type.Union([Type.String(), Type.Null()])");
  });

  test("nullable columns without default are Optional in Insert", () => {
    const schema = generator.generateInsertSchema(usersTable, ir);
    expect(schema).toContain("Type.Optional(");
  });

  test("generates enum schema", () => {
    const enumDef: EnumDef = { name: "user_status", values: ["active", "inactive", "banned"] };
    const schema = generator.generateEnumSchema(enumDef);
    expect(schema).toContain("UserStatus");
    expect(schema).toContain("Type.Union(");
    expect(schema).toContain('Type.Literal("active")');
    expect(schema).toContain('Type.Literal("inactive")');
    expect(schema).toContain('Type.Literal("banned")');
  });

  test("generates type alias with Prettify", () => {
    const alias = generator.generateTypeAlias("SelectUser", "SelectUser");
    expect(alias).toContain("Prettify<Static<typeof SelectUser>>");
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
bun test tests/generator/typescript/typebox.test.ts
```
Expected: FAIL

- [ ] **Step 3: Implement TypeBox generator**

Write `src/generator/typescript/schema/typebox.ts`. Must implement all `SchemaGenerator` methods:
- `generateImports()` → `import { Type, Static } from "typebox";`
- `generateEnumSchema()` → `Type.Union([Type.Literal("val1"), ...])`
- `generateSelectSchema()` → `Type.Object({...})` with ALL columns, nullable as `Type.Union([Type.X(), Type.Null()])`
- `generateInsertSchema()` → `Type.Object({...})` OMITTING columns with `hasDefault: true`, nullable without default as `Type.Optional(Type.Union([...]))`
- `generateTypeAlias()` → `export type X = Prettify<Static<typeof X>>;`

SQL category → TypeBox mapping:
- string → `Type.String()`
- number → `Type.Number()`
- boolean → `Type.Boolean()`
- date → `Type.Date()`
- json → `Type.Any()`
- uuid → `Type.String()`
- binary → `Type.Uint8Array()` (or `Type.String()` if unavailable)
- enum → `Type.Union([Type.Literal(...), ...])`  (look up `enumName` in IR)
- unknown → `Type.Unknown()`
- array with elementType → `Type.Array(elementTypeMapping)`

Use `pascalCase` from utils for table name → type name conversion.

- [ ] **Step 4: Run tests**

```bash
bun test tests/generator/typescript/typebox.test.ts
```
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/generator/typescript/schema/ tests/generator/typescript/typebox.test.ts
git commit -m "feat: TypeBox v1.0 schema generator with Select/Insert separation"
```

---

### Task 7: Bun.sql Driver Generator

**Branch:** `feat/07-bun-sql-driver` (from `main` after Task 5 merged)
**Depends on:** Task 5 | **Can parallel with:** Task 6
**Files:**
- Create: `src/generator/typescript/driver/bun-sql.ts`
- Create: `tests/generator/typescript/bun-sql.test.ts`

- [ ] **Step 1: Write driver generator tests**

Write `tests/generator/typescript/bun-sql.test.ts`:
```ts
import { describe, expect, test } from "bun:test";
import { createBunSqlGenerator } from "@/generator/typescript/driver/bun-sql";
import type { QueryDef } from "@/ir";

const generator = createBunSqlGenerator();

describe("Bun.sql Driver Generator", () => {
  test("generates client interface", () => {
    const iface = generator.generateClientInterface();
    expect(iface).toContain("DatabaseClient");
    expect(iface).toContain("query<T>");
    expect(iface).toContain("queryOne<T>");
    expect(iface).toContain("execute");
  });

  test("generates client adapter", () => {
    const adapter = generator.generateClientAdapter();
    expect(adapter).toContain("BunSqlClient");
    expect(adapter).toContain("implements DatabaseClient");
    expect(adapter).toContain("Bun.sql");
  });

  test("generates :one query function", () => {
    const query: QueryDef = {
      name: "GetUser",
      command: "one",
      sql: "SELECT * FROM users WHERE id = $1",
      params: [{ index: 1, name: "id", type: { raw: "INTEGER", normalized: "integer", category: "number" } }],
      returns: [
        { name: "id", type: { raw: "SERIAL", normalized: "serial", category: "number" }, nullable: false, hasDefault: true },
        { name: "name", type: { raw: "TEXT", normalized: "text", category: "string" }, nullable: false, hasDefault: false },
      ],
      sourceFile: "queries/users.sql",
    };
    const fn = generator.generateQueryFunction(query);
    expect(fn).toContain("getUser");
    expect(fn).toContain("Promise<SelectUser | null>");
    expect(fn).toContain("queryOne");
    expect(fn).toContain("params.id");
  });

  test("generates :many query function", () => {
    const query: QueryDef = {
      name: "ListUsers",
      command: "many",
      sql: "SELECT id, name FROM users",
      params: [],
      returns: [
        { name: "id", type: { raw: "SERIAL", normalized: "serial", category: "number" }, nullable: false, hasDefault: true },
      ],
      sourceFile: "queries/users.sql",
    };
    const fn = generator.generateQueryFunction(query);
    expect(fn).toContain("listUsers");
    expect(fn).toContain("Promise<ListUsersRow[]>");
    expect(fn).toContain("client.query");
  });

  test("generates :exec query function with void return", () => {
    const query: QueryDef = {
      name: "CreateUser",
      command: "exec",
      sql: "INSERT INTO users (name) VALUES ($1)",
      params: [{ index: 1, name: "name", type: { raw: "TEXT", normalized: "text", category: "string" } }],
      returns: [],
      sourceFile: "queries/users.sql",
    };
    const fn = generator.generateQueryFunction(query);
    expect(fn).toContain("createUser");
    expect(fn).toContain("Promise<void>");
    expect(fn).toContain("client.execute");
  });

  test("generates :execresult query function", () => {
    const query: QueryDef = {
      name: "DeleteUser",
      command: "execresult",
      sql: "DELETE FROM users WHERE id = $1",
      params: [{ index: 1, name: "id", type: { raw: "INTEGER", normalized: "integer", category: "number" } }],
      returns: [],
      sourceFile: "queries/users.sql",
    };
    const fn = generator.generateQueryFunction(query);
    expect(fn).toContain("deleteUser");
    expect(fn).toContain("rowsAffected");
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
bun test tests/generator/typescript/bun-sql.test.ts
```
Expected: FAIL

- [ ] **Step 3: Implement Bun.sql driver generator**

Write `src/generator/typescript/driver/bun-sql.ts`. Must implement:
- `generateImports()` → empty (Bun.sql is a global)
- `generateClientInterface()` → the `DatabaseClient` interface with `query<T>`, `queryOne<T>`, `execute`
- `generateClientAdapter()` → `BunSqlClient` class implementing `DatabaseClient` using `Bun.sql`
- `generateQueryFunction(query)` → async function using `camelCase(query.name)`, typed params object, correct return type based on command

Return type rules:
- `:one` → `Promise<SelectX | null>` (uses table schema type)
- `:many` → `Promise<QueryNameRow[]>` (generates inline Row type for explicit columns, or uses table type for `SELECT *`)
- `:exec` → `Promise<void>`
- `:execresult` → `Promise<{ rowsAffected: number }>`

Use `camelCase` for function names, `pascalCase` for type names.

- [ ] **Step 4: Run tests**

```bash
bun test tests/generator/typescript/bun-sql.test.ts
```
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/generator/typescript/driver/ tests/generator/typescript/bun-sql.test.ts
git commit -m "feat: Bun.sql driver generator with typed query functions"
```

---

### Task 8: TypeScript Language Plugin (Orchestrator)

**Branch:** `feat/08-ts-plugin` (from `main` after Tasks 6 + 7 merged)
**Depends on:** Tasks 6, 7
**Files:**
- Create: `src/generator/typescript/index.ts`
- Create: `tests/generator/typescript/plugin.test.ts`

- [ ] **Step 1: Write plugin integration tests**

Write `tests/generator/typescript/plugin.test.ts`:
```ts
import { describe, expect, test } from "bun:test";
import { createTypeScriptPlugin } from "@/generator/typescript";
import { createTypeBoxGenerator } from "@/generator/typescript/schema/typebox";
import { createBunSqlGenerator } from "@/generator/typescript/driver/bun-sql";
import type { SqlcxIR } from "@/ir";

const ir: SqlcxIR = {
  tables: [
    {
      name: "users",
      columns: [
        { name: "id", type: { raw: "SERIAL", normalized: "serial", category: "number" }, nullable: false, hasDefault: true },
        { name: "name", type: { raw: "TEXT", normalized: "text", category: "string" }, nullable: false, hasDefault: false },
        { name: "email", type: { raw: "TEXT", normalized: "text", category: "string" }, nullable: false, hasDefault: false },
        { name: "bio", type: { raw: "TEXT", normalized: "text", category: "string" }, nullable: true, hasDefault: false },
      ],
      primaryKey: ["id"],
      uniqueConstraints: [["email"]],
    },
  ],
  queries: [
    {
      name: "GetUser",
      command: "one",
      sql: "SELECT * FROM users WHERE id = $1",
      params: [{ index: 1, name: "id", type: { raw: "INTEGER", normalized: "integer", category: "number" } }],
      returns: [
        { name: "id", type: { raw: "SERIAL", normalized: "serial", category: "number" }, nullable: false, hasDefault: true },
        { name: "name", type: { raw: "TEXT", normalized: "text", category: "string" }, nullable: false, hasDefault: false },
      ],
      sourceFile: "queries/users.sql",
    },
  ],
  enums: [{ name: "user_status", values: ["active", "inactive"] }],
};

describe("TypeScript Language Plugin", () => {
  const plugin = createTypeScriptPlugin({
    schema: createTypeBoxGenerator(),
    driver: createBunSqlGenerator(),
  });

  test("generates expected files", () => {
    const files = plugin.generate(ir, { out: "./src/db" });
    const paths = files.map((f) => f.path);
    expect(paths).toContain("./src/db/schema.ts");
    expect(paths).toContain("./src/db/client.ts");
    expect(paths).toContain("./src/db/users.queries.ts");
  });

  test("schema.ts contains types and schemas", () => {
    const files = plugin.generate(ir, { out: "./src/db" });
    const schema = files.find((f) => f.path.endsWith("schema.ts"))!;
    expect(schema.content).toContain("SelectUser");
    expect(schema.content).toContain("InsertUser");
    expect(schema.content).toContain("UserStatus");  // enum
    expect(schema.content).toContain("Prettify");
  });

  test("client.ts contains DatabaseClient interface", () => {
    const files = plugin.generate(ir, { out: "./src/db" });
    const client = files.find((f) => f.path.endsWith("client.ts"))!;
    expect(client.content).toContain("DatabaseClient");
  });

  test("queries file contains typed functions", () => {
    const files = plugin.generate(ir, { out: "./src/db" });
    const queries = files.find((f) => f.path.endsWith("users.queries.ts"))!;
    expect(queries.content).toContain("getUser");
    expect(queries.content).toContain("DatabaseClient");
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
bun test tests/generator/typescript/plugin.test.ts
```
Expected: FAIL

- [ ] **Step 3: Implement TypeScript plugin**

Write `src/generator/typescript/index.ts`. This orchestrator:
1. Groups queries by source file (e.g., all queries from `users.sql` → `users.queries.ts`)
2. Generates `schema.ts` by iterating all tables + enums through the SchemaGenerator
3. Generates `client.ts` from the DriverGenerator
4. Generates one `<table>.queries.ts` per source file from the DriverGenerator
5. Adds a `Prettify` utility type export to `schema.ts`
6. Returns array of `GeneratedFile`

- [ ] **Step 4: Run tests**

```bash
bun test tests/generator/typescript/plugin.test.ts
```
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/generator/typescript/index.ts tests/generator/typescript/plugin.test.ts
git commit -m "feat: TypeScript language plugin orchestrating schema + driver generation"
```

---

### Task 9: Config Loading

**Branch:** `feat/09-config` (from `main` after Task 8 merged)
**Depends on:** Tasks 4, 5
**Files:**
- Create: `src/config/index.ts`
- Create: `tests/config/config.test.ts`

- [ ] **Step 1: Write config tests**

Write `tests/config/config.test.ts`:
```ts
import { describe, expect, test } from "bun:test";
import { defineConfig, type SqlcxConfig } from "@/config";

describe("defineConfig", () => {
  test("returns config as-is (type helper)", () => {
    const config = defineConfig({
      sql: "./sql",
      parser: { dialect: "postgres", parseSchema: () => [], parseQueries: () => [], parseEnums: () => [] },
      targets: [],
    });
    expect(config.sql).toBe("./sql");
  });

  test("overrides are optional", () => {
    const config = defineConfig({
      sql: "./sql",
      parser: { dialect: "postgres", parseSchema: () => [], parseQueries: () => [], parseEnums: () => [] },
      targets: [],
    });
    expect(config.overrides).toBeUndefined();
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
bun test tests/config/config.test.ts
```
Expected: FAIL

- [ ] **Step 3: Implement config**

Write `src/config/index.ts`:
```ts
import type { DatabaseParser } from "@/parser/interface";
import type { LanguagePlugin } from "@/generator/interface";

export interface SqlcxConfig {
  sql: string;
  parser: DatabaseParser;
  targets: LanguagePlugin[];
  overrides?: Record<string, string>;
}

export function defineConfig(config: SqlcxConfig): SqlcxConfig {
  return config;
}

export async function loadConfig(configPath: string): Promise<SqlcxConfig> {
  const mod = await import(configPath);
  return mod.default as SqlcxConfig;
}
```

- [ ] **Step 4: Run tests**

```bash
bun test tests/config/config.test.ts
```
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/config/ tests/config/
git commit -m "feat: config loading with defineConfig and dynamic import"
```

---

### Task 10: IR Caching

**Branch:** `feat/10-cache` (from `main` after Task 9 merged)
**Depends on:** Task 2
**Files:**
- Create: `src/cache/index.ts`
- Create: `tests/cache/cache.test.ts`

- [ ] **Step 1: Write cache tests**

Write `tests/cache/cache.test.ts`:
```ts
import { describe, expect, test, beforeEach, afterEach } from "bun:test";
import { computeHash, readCache, writeCache } from "@/cache";
import type { SqlcxIR } from "@/ir";
import { mkdirSync, rmSync, existsSync } from "fs";
import { join } from "path";

const testCacheDir = join(import.meta.dir, ".test-sqlcx");

beforeEach(() => {
  if (existsSync(testCacheDir)) rmSync(testCacheDir, { recursive: true });
});

afterEach(() => {
  if (existsSync(testCacheDir)) rmSync(testCacheDir, { recursive: true });
});

describe("IR Cache", () => {
  test("computeHash produces consistent SHA-256", () => {
    const files = [{ path: "a.sql", content: "CREATE TABLE a ();" }];
    const hash1 = computeHash(files);
    const hash2 = computeHash(files);
    expect(hash1).toBe(hash2);
    expect(hash1).toHaveLength(64); // SHA-256 hex
  });

  test("computeHash changes when content changes", () => {
    const hash1 = computeHash([{ path: "a.sql", content: "v1" }]);
    const hash2 = computeHash([{ path: "a.sql", content: "v2" }]);
    expect(hash1).not.toBe(hash2);
  });

  test("write and read cache round-trip", () => {
    const ir: SqlcxIR = { tables: [], queries: [], enums: [] };
    writeCache(testCacheDir, ir, "abc123");
    const result = readCache(testCacheDir, "abc123");
    expect(result).not.toBeNull();
    expect(result!.tables).toEqual([]);
  });

  test("read returns null on hash mismatch", () => {
    const ir: SqlcxIR = { tables: [], queries: [], enums: [] };
    writeCache(testCacheDir, ir, "abc123");
    const result = readCache(testCacheDir, "different");
    expect(result).toBeNull();
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
bun test tests/cache/cache.test.ts
```
Expected: FAIL

- [ ] **Step 3: Implement cache**

Write `src/cache/index.ts`:
```ts
import { createHash } from "crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { join } from "path";
import type { SqlcxIR } from "@/ir";

interface CacheFile {
  hash: string;
  ir: SqlcxIR;
}

export function computeHash(files: { path: string; content: string }[]): string {
  const sorted = [...files].sort((a, b) => a.path.localeCompare(b.path));
  const combined = sorted.map((f) => f.content).join("\n");
  return createHash("sha256").update(combined).digest("hex");
}

export function writeCache(cacheDir: string, ir: SqlcxIR, hash: string): void {
  if (!existsSync(cacheDir)) mkdirSync(cacheDir, { recursive: true });
  const data: CacheFile = { hash, ir };
  writeFileSync(join(cacheDir, "ir.json"), JSON.stringify(data, null, 2));
}

export function readCache(cacheDir: string, expectedHash: string): SqlcxIR | null {
  const cachePath = join(cacheDir, "ir.json");
  if (!existsSync(cachePath)) return null;
  const data: CacheFile = JSON.parse(readFileSync(cachePath, "utf-8"));
  if (data.hash !== expectedHash) return null;
  return data.ir;
}
```

- [ ] **Step 4: Run tests**

```bash
bun test tests/cache/cache.test.ts
```
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/cache/ tests/cache/
git commit -m "feat: IR caching with SHA-256 content hash invalidation"
```

---

### Task 11: CLI Entry Point

**Branch:** `feat/11-cli` (from `main` after Task 10 merged)
**Depends on:** Tasks 4, 8, 9, 10
**Files:**
- Create: `src/cli/index.ts`
- Create: `tests/cli/cli.test.ts`

- [ ] **Step 1: Write CLI tests**

Write `tests/cli/cli.test.ts`:
```ts
import { describe, expect, test, beforeEach, afterEach } from "bun:test";
import { mkdirSync, rmSync, writeFileSync, existsSync, readFileSync } from "fs";
import { join } from "path";

const testDir = join(import.meta.dir, ".test-cli");

beforeEach(() => {
  if (existsSync(testDir)) rmSync(testDir, { recursive: true });
  mkdirSync(testDir, { recursive: true });
  mkdirSync(join(testDir, "sql/queries"), { recursive: true });
  mkdirSync(join(testDir, "src/db"), { recursive: true });

  writeFileSync(
    join(testDir, "sql/schema.sql"),
    `CREATE TABLE users (
      id SERIAL PRIMARY KEY,
      name TEXT NOT NULL,
      email TEXT NOT NULL UNIQUE
    );`
  );

  writeFileSync(
    join(testDir, "sql/queries/users.sql"),
    `-- name: GetUser :one
SELECT * FROM users WHERE id = $1;

-- name: CreateUser :exec
INSERT INTO users (name, email) VALUES ($1, $2);`
  );
});

afterEach(() => {
  if (existsSync(testDir)) rmSync(testDir, { recursive: true });
});

describe("CLI generate", () => {
  test("generates output files from SQL fixtures", async () => {
    // Use the generate function directly (not spawning CLI process)
    const { generate } = await import("@/cli");
    await generate({
      sqlDir: join(testDir, "sql"),
      outDir: join(testDir, "src/db"),
      cacheDir: join(testDir, ".sqlcx"),
    });

    expect(existsSync(join(testDir, "src/db/schema.ts"))).toBe(true);
    expect(existsSync(join(testDir, "src/db/client.ts"))).toBe(true);
    expect(existsSync(join(testDir, "src/db/users.queries.ts"))).toBe(true);

    const schema = readFileSync(join(testDir, "src/db/schema.ts"), "utf-8");
    expect(schema).toContain("SelectUser");
    expect(schema).toContain("InsertUser");

    const queries = readFileSync(join(testDir, "src/db/users.queries.ts"), "utf-8");
    expect(queries).toContain("getUser");
    expect(queries).toContain("createUser");
  });

  test("creates cache file", async () => {
    const { generate } = await import("@/cli");
    await generate({
      sqlDir: join(testDir, "sql"),
      outDir: join(testDir, "src/db"),
      cacheDir: join(testDir, ".sqlcx"),
    });

    expect(existsSync(join(testDir, ".sqlcx/ir.json"))).toBe(true);
  });
});

describe("CLI check", () => {
  test("validates without writing output files", async () => {
    const { check } = await import("@/cli");
    const result = await check({
      sqlDir: join(testDir, "sql"),
      cacheDir: join(testDir, ".sqlcx"),
    });

    expect(result.valid).toBe(true);
    expect(result.tables).toBeGreaterThan(0);
    // Should NOT create output files
    expect(existsSync(join(testDir, "src/db/schema.ts"))).toBe(false);
  });

  test("returns errors for bad SQL", async () => {
    writeFileSync(join(testDir, "sql/bad.sql"), "CREATE TABL broken syntax;");
    const { check } = await import("@/cli");
    const result = await check({
      sqlDir: join(testDir, "sql"),
      cacheDir: join(testDir, ".sqlcx"),
    });

    expect(result.valid).toBe(false);
    expect(result.errors.length).toBeGreaterThan(0);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
bun test tests/cli/cli.test.ts
```
Expected: FAIL

- [ ] **Step 3: Implement CLI**

Write `src/cli/index.ts`. This is the orchestrator that ties everything together:

1. `generate({ sqlDir, outDir, cacheDir })` function:
   - Glob all `.sql` files from `sqlDir`
   - Separate schema files (containing `CREATE TABLE`) from query files (containing `-- name:`)
   - Compute content hash of all SQL files
   - Check cache — if hit, use cached IR; if miss, parse
   - Parse: call `parser.parseEnums()`, `parser.parseSchema()`, `parser.parseQueries()` for each query file
   - Build `SqlcxIR` object
   - Write cache
   - Call TypeScript plugin's `generate(ir, { out: outDir })` (hardcode for now — config loading is separate)
   - Write all `GeneratedFile`s to disk

2. CLI argument parsing (minimal, no library):
   - `sqlcx generate` → calls `generate()` with defaults or config
   - `sqlcx check` → calls `generate()` in dry-run mode (parse + validate, don't write)
   - `sqlcx init` → scaffold template files

3. Add `bin` field to `package.json` pointing to `src/cli/index.ts`

- [ ] **Step 4: Run tests**

```bash
bun test tests/cli/cli.test.ts
```
Expected: PASS

- [ ] **Step 5: Test CLI binary**

```bash
bun run src/cli/index.ts generate --help
```
Expected: prints usage info or runs with defaults

- [ ] **Step 6: Commit**

```bash
git add src/cli/ tests/cli/
git commit -m "feat: CLI entry point with generate, check, and init commands"
```

---

### Task 12: End-to-End Integration Test

**Branch:** `feat/12-e2e-tests` (from `main` after Task 11 merged)
**Depends on:** Task 11
**Files:**
- Create: `tests/e2e/generate.test.ts`

- [ ] **Step 1: Write E2E test**

Write `tests/e2e/generate.test.ts` that:
1. Creates a temp directory with realistic SQL schema + queries (users, posts tables, enums, JOINs)
2. Runs the full `generate()` pipeline
3. Asserts the generated TypeScript files are syntactically valid (import and check with Bun)
4. Asserts the generated types match expected structure
5. Asserts cache works on second run (faster, same output)

- [ ] **Step 2: Run E2E test**

```bash
bun test tests/e2e/generate.test.ts
```
Expected: PASS

- [ ] **Step 3: Run full test suite**

```bash
bun test
```
Expected: ALL PASS

- [ ] **Step 4: Commit**

```bash
git add tests/e2e/
git commit -m "feat: end-to-end integration test for full generate pipeline"
```

---

### Task 13: Package Finalization

**Branch:** `feat/13-package-finalization` (from `main` after Task 12 merged)
**Depends on:** All prior tasks
**Files:**
- Modify: `package.json` (add bin, exports, scripts)

- [ ] **Step 1: Update package.json**

Add to `package.json`:
```json
{
  "name": "sqlcx",
  "bin": {
    "sqlcx": "src/cli/index.ts"
  },
  "exports": {
    ".": "./src/index.ts",
    "./parser/postgres": "./src/parser/postgres.ts",
    "./lang/typescript": "./src/generator/typescript/index.ts",
    "./schema/typebox": "./src/generator/typescript/schema/typebox.ts",
    "./driver/bun-sql": "./src/generator/typescript/driver/bun-sql.ts"
  },
  "scripts": {
    "test": "bun test",
    "generate": "bun run src/cli/index.ts generate",
    "check": "bun run src/cli/index.ts check"
  }
}
```

- [ ] **Step 2: Create main entry point**

Write `src/index.ts` that re-exports public API:
```ts
export { defineConfig } from "./config";
export type { SqlcxConfig } from "./config";
export type { SqlcxIR, TableDef, ColumnDef, QueryDef, ParamDef, EnumDef, SqlType } from "./ir/types";
export type { DatabaseParser } from "./parser/interface";
export type { LanguagePlugin, SchemaGenerator, DriverGenerator, GeneratedFile, LanguageOptions } from "./generator/interface";
```

- [ ] **Step 3: Run full test suite**

```bash
bun test
```
Expected: ALL PASS

- [ ] **Step 4: Test CLI via bun**

```bash
bunx sqlcx --help
```

- [ ] **Step 5: Commit**

```bash
git add package.json src/index.ts
git commit -m "feat: package exports, bin entry, and public API"
```
