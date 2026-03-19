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
    {
      name: "CreateUser",
      command: "exec",
      sql: "INSERT INTO users (name, email) VALUES ($1, $2)",
      params: [
        { index: 1, name: "name", type: { raw: "TEXT", normalized: "text", category: "string" } },
        { index: 2, name: "email", type: { raw: "TEXT", normalized: "text", category: "string" } },
      ],
      returns: [],
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
    // queries grouped by sourceFile
    expect(paths.some((p) => p.includes("users.queries.ts"))).toBe(true);
  });

  test("schema.ts contains Select/Insert schemas, enum, Prettify aliases", () => {
    const files = plugin.generate(ir, { out: "./src/db" });
    const schema = files.find((f) => f.path.endsWith("schema.ts"))!;
    expect(schema.content).toContain("SelectUsers");
    expect(schema.content).toContain("InsertUsers");
    expect(schema.content).toContain("UserStatus"); // enum
    expect(schema.content).toContain("Prettify");
    expect(schema.content).toContain("Static");
  });

  test("client.ts contains DatabaseClient interface", () => {
    const files = plugin.generate(ir, { out: "./src/db" });
    const client = files.find((f) => f.path.endsWith("client.ts"))!;
    expect(client.content).toContain("DatabaseClient");
    expect(client.content).toContain("query<T>");
    expect(client.content).toContain("queryOne<T>");
    expect(client.content).toContain("execute");
  });

  test("client.ts contains BunSqlClient adapter", () => {
    const files = plugin.generate(ir, { out: "./src/db" });
    const client = files.find((f) => f.path.endsWith("client.ts"))!;
    expect(client.content).toContain("BunSqlClient");
  });

  test("queries file contains typed functions", () => {
    const files = plugin.generate(ir, { out: "./src/db" });
    const queries = files.find((f) => f.path.includes("users.queries.ts"))!;
    expect(queries.content).toContain("getUser");
    expect(queries.content).toContain("createUser");
    expect(queries.content).toContain("DatabaseClient");
  });

  test("queries file imports from client.ts", () => {
    const files = plugin.generate(ir, { out: "./src/db" });
    const queries = files.find((f) => f.path.includes("users.queries.ts"))!;
    expect(queries.content).toContain("import");
    expect(queries.content).toContain("DatabaseClient");
  });

  test("multiple source files produce multiple query files", () => {
    const multiIR: SqlcxIR = {
      ...ir,
      queries: [
        ...ir.queries,
        {
          name: "GetPost",
          command: "one",
          sql: "SELECT * FROM posts WHERE id = $1",
          params: [{ index: 1, name: "id", type: { raw: "INTEGER", normalized: "integer", category: "number" } }],
          returns: [{ name: "id", type: { raw: "SERIAL", normalized: "serial", category: "number" }, nullable: false, hasDefault: true }],
          sourceFile: "queries/posts.sql",
        },
      ],
    };
    const files = plugin.generate(multiIR, { out: "./src/db" });
    const queryFiles = files.filter((f) => f.path.includes(".queries.ts"));
    expect(queryFiles.length).toBe(2);
    expect(queryFiles.some((f) => f.path.includes("users.queries.ts"))).toBe(true);
    expect(queryFiles.some((f) => f.path.includes("posts.queries.ts"))).toBe(true);
  });
});
