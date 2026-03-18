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

    test("detects columns with defaults", () => {
      const tables = parser.parseSchema(schemaSql);
      const users = tables.find((t) => t.name === "users")!;
      const createdAt = users.columns.find((c) => c.name === "created_at")!;
      expect(createdAt.hasDefault).toBe(true);
      const nameCol = users.columns.find((c) => c.name === "name")!;
      expect(nameCol.hasDefault).toBe(false);
    });

    test("detects enum typed columns", () => {
      const tables = parser.parseSchema(schemaSql);
      const users = tables.find((t) => t.name === "users")!;
      const statusCol = users.columns.find((c) => c.name === "status")!;
      expect(statusCol.type.category).toBe("enum");
      expect(statusCol.type.enumName).toBe("user_status");
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
      expect(queries).toHaveLength(5);
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
