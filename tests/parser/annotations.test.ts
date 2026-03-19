import { describe, expect, test } from "bun:test";
import { createPostgresParser } from "@/parser/postgres";

describe("Inline Annotations", () => {
  const parser = createPostgresParser();

  describe("@enum annotation", () => {
    test('parses @enum("admin", "user", "guest") → column gets enumValues', () => {
      const sql = `CREATE TABLE accounts (
  id SERIAL PRIMARY KEY,
  -- @enum("admin", "user", "guest")
  role TEXT NOT NULL DEFAULT 'user'
);`;
      const tables = parser.parseSchema(sql);
      const t = tables[0];
      const roleCol = t.columns.find((c) => c.name === "role")!;
      expect(roleCol.type.enumValues).toEqual(["admin", "user", "guest"]);
      expect(roleCol.type.category).toBe("enum");
    });
  });

  describe("@json annotation", () => {
    test("parses @json({ theme: string, lang: string }) → object shape", () => {
      const sql = `CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  -- @json({ theme: string, lang: string })
  metadata JSONB NOT NULL
);`;
      const tables = parser.parseSchema(sql);
      const col = tables[0].columns.find((c) => c.name === "metadata")!;
      expect(col.type.jsonShape).toEqual({
        kind: "object",
        fields: {
          theme: { kind: "string" },
          lang: { kind: "string" },
        },
      });
    });

    test("parses @json({ nested: { deep: boolean } }) → nested object shape", () => {
      const sql = `CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  -- @json({ nested: { deep: boolean } })
  config JSONB NOT NULL
);`;
      const tables = parser.parseSchema(sql);
      const col = tables[0].columns.find((c) => c.name === "config")!;
      expect(col.type.jsonShape).toEqual({
        kind: "object",
        fields: {
          nested: {
            kind: "object",
            fields: {
              deep: { kind: "boolean" },
            },
          },
        },
      });
    });

    test("parses @json(string[]) → array shape", () => {
      const sql = `CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  -- @json(string[])
  tags JSONB NOT NULL
);`;
      const tables = parser.parseSchema(sql);
      const col = tables[0].columns.find((c) => c.name === "tags")!;
      expect(col.type.jsonShape).toEqual({
        kind: "array",
        element: { kind: "string" },
      });
    });

    test("parses @json({ name: string, tags: string[] }) → mixed object with array field", () => {
      const sql = `CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  -- @json({ name: string, tags: string[] })
  profile JSONB NOT NULL
);`;
      const tables = parser.parseSchema(sql);
      const col = tables[0].columns.find((c) => c.name === "profile")!;
      expect(col.type.jsonShape).toEqual({
        kind: "object",
        fields: {
          name: { kind: "string" },
          tags: { kind: "array", element: { kind: "string" } },
        },
      });
    });

    test("parses @json(string?) → nullable shape", () => {
      const sql = `CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  -- @json(string?)
  nickname JSONB
);`;
      const tables = parser.parseSchema(sql);
      const col = tables[0].columns.find((c) => c.name === "nickname")!;
      expect(col.type.jsonShape).toEqual({
        kind: "nullable",
        inner: { kind: "string" },
      });
    });
  });

  describe("no annotation", () => {
    test("column without annotation has no enumValues/jsonShape", () => {
      const sql = `CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  name TEXT NOT NULL
);`;
      const tables = parser.parseSchema(sql);
      const col = tables[0].columns.find((c) => c.name === "name")!;
      expect(col.type.enumValues).toBeUndefined();
      expect(col.type.jsonShape).toBeUndefined();
    });
  });
});
