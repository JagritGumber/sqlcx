import { existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { join, dirname, basename, extname, relative } from "path";
import { computeHash, readCache, writeCache } from "@/cache";
import { createPostgresParser } from "@/parser/postgres";
import { createTypeScriptPlugin } from "@/generator/typescript";
import { createTypeBoxGenerator } from "@/generator/typescript/schema/typebox";
import { createBunSqlGenerator } from "@/generator/typescript/driver/bun-sql";
import type { SqlcxIR, TableDef, QueryDef, EnumDef } from "@/ir";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function globSqlFiles(dir: string): string[] {
  const glob = new Bun.Glob("**/*.sql");
  const paths: string[] = [];
  for (const match of glob.scanSync({ cwd: dir, absolute: true })) {
    paths.push(match);
  }
  return paths.sort();
}

function isSchemaFile(content: string): boolean {
  return /CREATE\s+TABLE/i.test(content);
}

function isQueryFile(content: string): boolean {
  return /--\s*name:/i.test(content);
}

function getFlag(args: string[], flag: string): string | undefined {
  const idx = args.indexOf(flag);
  return idx !== -1 && idx + 1 < args.length ? args[idx + 1] : undefined;
}

// ---------------------------------------------------------------------------
// Generate
// ---------------------------------------------------------------------------

export interface GenerateOptions {
  sqlDir: string;
  outDir: string;
  cacheDir: string;
}

export async function generate(options: GenerateOptions): Promise<void> {
  const { sqlDir, outDir, cacheDir } = options;

  // 1. Glob all .sql files
  const sqlFiles = globSqlFiles(sqlDir);
  if (sqlFiles.length === 0) {
    console.log("No .sql files found in", sqlDir);
    return;
  }

  // 2. Read all SQL files
  const fileContents = sqlFiles.map((f) => ({
    path: relative(sqlDir, f),
    content: readFileSync(f, "utf-8"),
  }));

  // 3. Compute content hash
  const hash = computeHash(fileContents);

  // 4. Check cache
  let ir = readCache(cacheDir, hash);

  if (!ir) {
    // 5. Parse
    ir = parse(fileContents);

    // 6. Write cache
    writeCache(cacheDir, ir, hash);
  }

  // 7. Generate output
  const plugin = createTypeScriptPlugin({
    schema: createTypeBoxGenerator(),
    driver: createBunSqlGenerator(),
  });
  const generatedFiles = plugin.generate(ir, { out: outDir });

  // 8. Write files to disk
  for (const file of generatedFiles) {
    const dir = dirname(file.path);
    if (!existsSync(dir)) {
      mkdirSync(dir, { recursive: true });
    }
    writeFileSync(file.path, file.content, "utf-8");
  }

  // 9. Log success
  console.log(`Generated ${generatedFiles.length} files to ${outDir}`);
}

// ---------------------------------------------------------------------------
// Check
// ---------------------------------------------------------------------------

export interface CheckOptions {
  sqlDir: string;
  cacheDir: string;
}

export interface CheckResult {
  valid: boolean;
  tables: number;
  queries: number;
  errors: string[];
}

export async function check(options: CheckOptions): Promise<CheckResult> {
  const { sqlDir, cacheDir } = options;
  const errors: string[] = [];

  // 1. Glob all .sql files
  const sqlFiles = globSqlFiles(sqlDir);
  if (sqlFiles.length === 0) {
    return { valid: true, tables: 0, queries: 0, errors: [] };
  }

  // 2. Read all SQL files
  const fileContents = sqlFiles.map((f) => ({
    path: relative(sqlDir, f),
    content: readFileSync(f, "utf-8"),
  }));

  // 3. Compute hash and check cache
  const hash = computeHash(fileContents);
  let ir = readCache(cacheDir, hash);

  if (!ir) {
    // 4. Parse
    try {
      ir = parse(fileContents);
      writeCache(cacheDir, ir, hash);
    } catch (err) {
      errors.push(String(err));
      return { valid: false, tables: 0, queries: 0, errors };
    }
  }

  return {
    valid: errors.length === 0,
    tables: ir.tables.length,
    queries: ir.queries.length,
    errors,
  };
}

// ---------------------------------------------------------------------------
// Shared parse logic
// ---------------------------------------------------------------------------

function parse(
  fileContents: { path: string; content: string }[],
): SqlcxIR {
  const parser = createPostgresParser();

  const schemaFiles = fileContents.filter((f) => isSchemaFile(f.content));
  const queryFiles = fileContents.filter((f) => isQueryFile(f.content));

  // Parse enums from all schema files
  const allSchemaSql = schemaFiles.map((f) => f.content).join("\n\n");
  const enums: EnumDef[] = parser.parseEnums(allSchemaSql);

  // Parse tables from all schema files
  const tables: TableDef[] = parser.parseSchema(allSchemaSql);

  // Parse queries from each query file
  const queries: QueryDef[] = [];
  for (const file of queryFiles) {
    const parsed = parser.parseQueries(file.content, tables);
    for (const q of parsed) {
      q.sourceFile = basename(file.path, extname(file.path));
      queries.push(q);
    }
  }

  return { tables, queries, enums };
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

async function init(): Promise<void> {
  // Create sql/ directory with example files
  const sqlDir = "sql";
  const queriesDir = join(sqlDir, "queries");

  if (!existsSync(sqlDir)) {
    mkdirSync(queriesDir, { recursive: true });
  } else if (!existsSync(queriesDir)) {
    mkdirSync(queriesDir, { recursive: true });
  }

  const schemaPath = join(sqlDir, "schema.sql");
  if (!existsSync(schemaPath)) {
    writeFileSync(
      schemaPath,
      `CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  email TEXT NOT NULL UNIQUE,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
`,
      "utf-8",
    );
    console.log("Created", schemaPath);
  }

  const queryPath = join(queriesDir, "users.sql");
  if (!existsSync(queryPath)) {
    writeFileSync(
      queryPath,
      `-- name: GetUserById :one
SELECT * FROM users WHERE id = $1;

-- name: ListUsers :many
SELECT * FROM users ORDER BY created_at DESC;

-- name: CreateUser :exec
INSERT INTO users (name, email) VALUES ($1, $2);
`,
      "utf-8",
    );
    console.log("Created", queryPath);
  }

  const configPath = "sqlcx.config.ts";
  if (!existsSync(configPath)) {
    writeFileSync(
      configPath,
      `import { defineConfig } from "sqlcx";
import { createPostgresParser } from "sqlcx/parser/postgres";
import { createTypeScriptPlugin } from "sqlcx/generator/typescript";
import { createTypeBoxGenerator } from "sqlcx/generator/typescript/schema/typebox";
import { createBunSqlGenerator } from "sqlcx/generator/typescript/driver/bun-sql";

export default defineConfig({
  sql: "./sql",
  parser: createPostgresParser(),
  targets: [
    createTypeScriptPlugin({
      schema: createTypeBoxGenerator(),
      driver: createBunSqlGenerator(),
    }),
  ],
});
`,
      "utf-8",
    );
    console.log("Created", configPath);
  }

  console.log("\nProject initialized! Run 'sqlcx generate' to generate types.");
}

// ---------------------------------------------------------------------------
// CLI entry point
// ---------------------------------------------------------------------------

const args = process.argv.slice(2);
const command = args[0];

if (command === "generate") {
  await generate({
    sqlDir: getFlag(args, "--sql") ?? "./sql",
    outDir: getFlag(args, "--out") ?? "./src/db",
    cacheDir: getFlag(args, "--cache") ?? ".sqlcx",
  });
} else if (command === "check") {
  const result = await check({
    sqlDir: getFlag(args, "--sql") ?? "./sql",
    cacheDir: getFlag(args, "--cache") ?? ".sqlcx",
  });
  if (!result.valid) {
    console.error("Check failed:", result.errors);
    process.exit(1);
  }
  console.log(
    `Check passed: ${result.tables} tables, ${result.queries} queries`,
  );
} else if (command === "init") {
  await init();
} else if (command !== undefined) {
  console.log("Usage: sqlcx <generate|check|init> [options]");
  console.log("  --sql <dir>    SQL directory (default: ./sql)");
  console.log("  --out <dir>    Output directory (default: ./src/db)");
  console.log("  --cache <dir>  Cache directory (default: .sqlcx)");
}
