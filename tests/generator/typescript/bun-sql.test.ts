import { describe, expect, test } from "bun:test";
import { createBunSqlGenerator } from "@/generator/typescript/driver/bun-sql";
import type { QueryDef } from "@/ir";

const generator = createBunSqlGenerator();

describe("Bun.sql Driver Generator", () => {
  test("generates client adapter", () => {
    const adapter = generator.generateClientAdapter();
    expect(adapter).toContain("BunSqlClient");
    expect(adapter).toContain("DatabaseClient");
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
    expect(fn).toContain("| null");
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
        { name: "name", type: { raw: "TEXT", normalized: "text", category: "string" }, nullable: false, hasDefault: false },
      ],
      sourceFile: "queries/users.sql",
    };
    const fn = generator.generateQueryFunction(query);
    expect(fn).toContain("listUsers");
    expect(fn).toContain("[]");
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

  test("generates no-param function without params argument", () => {
    const query: QueryDef = {
      name: "CountUsers",
      command: "one",
      sql: "SELECT COUNT(*) as count FROM users",
      params: [],
      returns: [
        { name: "count", type: { raw: "BIGINT", normalized: "bigint", category: "number" }, nullable: false, hasDefault: false },
      ],
      sourceFile: "queries/users.sql",
    };
    const fn = generator.generateQueryFunction(query);
    expect(fn).toContain("countUsers");
    expect(fn).not.toContain("Params");
  });

  test("array columns generate array TypeScript type", () => {
    const query: QueryDef = {
      name: "GetUserTags",
      command: "one",
      sql: "SELECT tags FROM users WHERE id = $1",
      params: [{ index: 1, name: "id", type: { raw: "INTEGER", normalized: "integer", category: "number" } }],
      returns: [
        { name: "tags", type: { raw: "TEXT[]", normalized: "text[]", category: "string", elementType: { raw: "TEXT", normalized: "text", category: "string" } }, nullable: true, hasDefault: false },
      ],
      sourceFile: "queries/users.sql",
    };
    const fn = generator.generateQueryFunction(query);
    expect(fn).toContain("string[]");
  });

  test("aliased columns use alias as field name", () => {
    const query: QueryDef = {
      name: "GetUserAlias",
      command: "one",
      sql: "SELECT id AS user_id FROM users WHERE id = $1",
      params: [{ index: 1, name: "id", type: { raw: "INTEGER", normalized: "integer", category: "number" } }],
      returns: [
        { name: "id", alias: "user_id", type: { raw: "SERIAL", normalized: "serial", category: "number" }, nullable: false, hasDefault: true },
      ],
      sourceFile: "queries/users.sql",
    };
    const fn = generator.generateQueryFunction(query);
    expect(fn).toContain("user_id: number");
    // Row type should use alias, not original name
    expect(fn).toMatch(/interface GetUserAliasRow \{[^}]*user_id: number/);
  });

  test("row type and params type are exported", () => {
    const query: QueryDef = {
      name: "GetUser",
      command: "one",
      sql: "SELECT * FROM users WHERE id = $1",
      params: [{ index: 1, name: "id", type: { raw: "INTEGER", normalized: "integer", category: "number" } }],
      returns: [
        { name: "id", type: { raw: "SERIAL", normalized: "serial", category: "number" }, nullable: false, hasDefault: true },
      ],
      sourceFile: "queries/users.sql",
    };
    const fn = generator.generateQueryFunction(query);
    expect(fn).toContain("export interface GetUserRow");
    expect(fn).toContain("export interface GetUserParams");
  });

  test("SQL uses JSON.stringify for safe embedding", () => {
    const query: QueryDef = {
      name: "Simple",
      command: "exec",
      sql: "DELETE FROM users",
      params: [],
      returns: [],
      sourceFile: "queries/users.sql",
    };
    const fn = generator.generateQueryFunction(query);
    expect(fn).toContain('const simpleSql = "DELETE FROM users"');
  });

  test("multi-line SQL is properly escaped", () => {
    const query: QueryDef = {
      name: "GetActiveUsers",
      command: "many",
      sql: "SELECT *\nFROM users\nWHERE status = 'active'",
      params: [],
      returns: [
        { name: "id", type: { raw: "SERIAL", normalized: "serial", category: "number" }, nullable: false, hasDefault: true },
      ],
      sourceFile: "queries/users.sql",
    };
    const fn = generator.generateQueryFunction(query);
    // Should use escaped newlines, not literal newlines
    expect(fn).toContain("\\n");
    // Should handle single quotes inside SQL
    expect(fn).toContain("active");
    // The const line should be valid JS (no unescaped newlines)
    const sqlLine = fn.split("\n").find((l: string) => l.includes("getActiveUsersSql"));
    expect(sqlLine).toBeDefined();
    // Should not contain a literal unescaped newline within the string
    expect(sqlLine).not.toMatch(/= ".*\n/);
  });
});
