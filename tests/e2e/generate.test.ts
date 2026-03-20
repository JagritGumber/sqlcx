import { describe, expect, test, beforeEach, afterEach } from "bun:test";
import {
  mkdirSync,
  rmSync,
  writeFileSync,
  existsSync,
  readFileSync,
} from "fs";
import { join } from "path";
import { generate, check } from "@/cli";

const testDir = join(import.meta.dir, ".test-e2e");
const sqlDir = join(testDir, "sql");
const queriesDir = join(sqlDir, "queries");
const outDir = join(testDir, "src/db");
const cacheDir = join(testDir, ".sqlcx");

const schemaSql = `\
CREATE TYPE post_status AS ENUM ('draft', 'published', 'archived');

CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  username TEXT NOT NULL UNIQUE,
  email TEXT NOT NULL,
  -- @json({ theme: string, notifications: boolean })
  preferences JSONB,
  tags TEXT[],
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE posts (
  id SERIAL PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id),
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  status post_status NOT NULL DEFAULT 'draft',
  -- @json({ views: number, likes: number })
  stats JSONB NOT NULL,
  published_at TIMESTAMP
);
`;

const userQueriesSql = `\
-- name: GetUser :one
SELECT * FROM users WHERE id = $1;

-- name: ListUsers :many
SELECT id, username, email FROM users ORDER BY created_at DESC;

-- name: CreateUser :one
INSERT INTO users (username, email) VALUES ($1, $2) RETURNING *;
`;

const postQueriesSql = `\
-- name: GetPost :one
SELECT * FROM posts WHERE id = $1;

-- name: ListPostsByUser :many
SELECT * FROM posts WHERE user_id = $1 AND status = $2;

-- name: CreatePost :exec
INSERT INTO posts (user_id, title, body) VALUES ($1, $2, $3);

-- name: DeletePost :execresult
DELETE FROM posts WHERE id = $1;

-- name: ListPostsByDateRange :many
-- @param $1 start_date
-- @param $2 end_date
SELECT * FROM posts WHERE published_at > $1 AND published_at < $2;
`;

function opts() {
  return { sqlDir, outDir, cacheDir };
}

function readOut(file: string): string {
  return readFileSync(join(outDir, file), "utf-8");
}

beforeEach(() => {
  if (existsSync(testDir)) rmSync(testDir, { recursive: true });
  mkdirSync(queriesDir, { recursive: true });
  mkdirSync(outDir, { recursive: true });

  writeFileSync(join(sqlDir, "schema.sql"), schemaSql);
  writeFileSync(join(queriesDir, "users.sql"), userQueriesSql);
  writeFileSync(join(queriesDir, "posts.sql"), postQueriesSql);
});

afterEach(() => {
  if (existsSync(testDir)) rmSync(testDir, { recursive: true });
});

// ---------------------------------------------------------------------------
// Full pipeline
// ---------------------------------------------------------------------------

describe("E2E generate pipeline", () => {
  test("generates schema.ts, client.ts, and query files", async () => {
    await generate(opts());

    expect(existsSync(join(outDir, "schema.ts"))).toBe(true);
    expect(existsSync(join(outDir, "client.ts"))).toBe(true);
    expect(existsSync(join(outDir, "users.queries.ts"))).toBe(true);
    expect(existsSync(join(outDir, "posts.queries.ts"))).toBe(true);
  });

  // ---- schema.ts assertions ----

  test("schema.ts contains Select/Insert schemas for each table", async () => {
    await generate(opts());
    const schema = readOut("schema.ts");

    expect(schema).toContain("SelectUsers");
    expect(schema).toContain("InsertUsers");
    expect(schema).toContain("SelectPosts");
    expect(schema).toContain("InsertPosts");
  });

  test("schema.ts contains enum schema", async () => {
    await generate(opts());
    const schema = readOut("schema.ts");

    expect(schema).toContain("PostStatus");
    expect(schema).toContain('Type.Literal("draft")');
    expect(schema).toContain('Type.Literal("published")');
    expect(schema).toContain('Type.Literal("archived")');
  });

  test("schema.ts contains Prettify type aliases", async () => {
    await generate(opts());
    const schema = readOut("schema.ts");

    expect(schema).toContain("Prettify");
    expect(schema).toContain("Static");
  });

  test("schema.ts has typed JSON shapes (not unknown)", async () => {
    await generate(opts());
    const schema = readOut("schema.ts");

    // preferences: @json({ theme: string, notifications: boolean })
    expect(schema).toContain('"theme": Type.String()');
    expect(schema).toContain('"notifications": Type.Boolean()');
    // stats: @json({ views: number, likes: number })
    expect(schema).toContain('"views": Type.Number()');
    expect(schema).toContain('"likes": Type.Number()');
  });

  test("schema.ts has array columns using Type.Array", async () => {
    await generate(opts());
    const schema = readOut("schema.ts");

    expect(schema).toContain("Type.Array(Type.String())");
  });

  // ---- client.ts assertions ----

  test("client.ts contains DatabaseClient interface", async () => {
    await generate(opts());
    const client = readOut("client.ts");

    expect(client).toContain("DatabaseClient");
    expect(client).toContain("query<T>");
    expect(client).toContain("queryOne<T>");
    expect(client).toContain("execute");
  });

  test("client.ts contains BunSqlClient adapter", async () => {
    await generate(opts());
    const client = readOut("client.ts");

    expect(client).toContain("BunSqlClient");
  });

  // ---- query file assertions ----

  test("users.queries.ts contains correctly named functions", async () => {
    await generate(opts());
    const queries = readOut("users.queries.ts");

    expect(queries).toContain("getUser");
    expect(queries).toContain("listUsers");
    expect(queries).toContain("createUser");
  });

  test("posts.queries.ts contains correctly named functions", async () => {
    await generate(opts());
    const queries = readOut("posts.queries.ts");

    expect(queries).toContain("getPost");
    expect(queries).toContain("listPostsByUser");
    expect(queries).toContain("createPost");
    expect(queries).toContain("deletePost");
    expect(queries).toContain("listPostsByDateRange");
  });

  test("query files import DatabaseClient from ./client", async () => {
    await generate(opts());
    const usersQ = readOut("users.queries.ts");
    const postsQ = readOut("posts.queries.ts");

    expect(usersQ).toContain("DatabaseClient");
    expect(usersQ).toContain("import");
    expect(postsQ).toContain("DatabaseClient");
    expect(postsQ).toContain("import");
  });

  test("query files have typed params and return types", async () => {
    await generate(opts());
    const postsQ = readOut("posts.queries.ts");

    // :one → returns Row | null
    expect(postsQ).toContain("| null");
    // :exec → Promise<void>
    expect(postsQ).toContain("Promise<void>");
    // :execresult → rowsAffected
    expect(postsQ).toContain("rowsAffected");
    // :many → []
    expect(postsQ).toContain("[]");
  });

  test("@param overrides produce named params", async () => {
    await generate(opts());
    const postsQ = readOut("posts.queries.ts");

    expect(postsQ).toContain("start_date");
    expect(postsQ).toContain("end_date");
  });

  test("RETURNING clause produces return columns", async () => {
    await generate(opts());
    const usersQ = readOut("users.queries.ts");

    // CreateUser uses RETURNING * → should have a Row type with user columns
    expect(usersQ).toContain("CreateUserRow");
  });
});

// ---------------------------------------------------------------------------
// Cache behavior
// ---------------------------------------------------------------------------

describe("E2E cache behavior", () => {
  test("generate creates cache file", async () => {
    await generate(opts());

    expect(existsSync(join(cacheDir, "ir.json"))).toBe(true);
  });

  test("second generate produces identical output (cache hit)", async () => {
    await generate(opts());
    const firstSchema = readOut("schema.ts");
    const firstUsers = readOut("users.queries.ts");
    const firstPosts = readOut("posts.queries.ts");

    await generate(opts());
    const secondSchema = readOut("schema.ts");
    const secondUsers = readOut("users.queries.ts");
    const secondPosts = readOut("posts.queries.ts");

    expect(firstSchema).toBe(secondSchema);
    expect(firstUsers).toBe(secondUsers);
    expect(firstPosts).toBe(secondPosts);
  });

  test("modifying SQL invalidates cache and updates output", async () => {
    await generate(opts());
    const firstPosts = readOut("posts.queries.ts");

    // Add a new query to posts.sql
    const updatedPostsSql =
      postQueriesSql +
      `\n-- name: CountPosts :one\nSELECT COUNT(*) as count FROM posts;\n`;
    writeFileSync(join(queriesDir, "posts.sql"), updatedPostsSql);

    await generate(opts());
    const secondPosts = readOut("posts.queries.ts");

    expect(secondPosts).not.toBe(firstPosts);
    expect(secondPosts).toContain("countPosts");
  });
});

// ---------------------------------------------------------------------------
// Check command
// ---------------------------------------------------------------------------

describe("E2E check command", () => {
  test("check returns valid: true without writing files", async () => {
    // Ensure outDir is empty before check
    if (existsSync(outDir)) rmSync(outDir, { recursive: true });
    mkdirSync(outDir, { recursive: true });

    const result = await check({ sqlDir, cacheDir });

    expect(result.valid).toBe(true);
    expect(result.tables).toBe(2);
    expect(result.queries).toBeGreaterThanOrEqual(8);

    // check() should NOT write generated output files
    expect(existsSync(join(outDir, "schema.ts"))).toBe(false);
    expect(existsSync(join(outDir, "client.ts"))).toBe(false);
  });
});
