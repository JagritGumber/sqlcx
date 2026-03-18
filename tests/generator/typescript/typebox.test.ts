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
    { name: "tags", type: { raw: "TEXT[]", normalized: "text[]", category: "string", elementType: { raw: "TEXT", normalized: "text", category: "string" } }, nullable: true, hasDefault: false },
    { name: "created_at", type: { raw: "TIMESTAMP", normalized: "timestamp", category: "date" }, nullable: false, hasDefault: true },
  ],
  primaryKey: ["id"],
  uniqueConstraints: [["email"]],
};

const ir: SqlcxIR = { tables: [usersTable], queries: [], enums: [] };

describe("TypeBox Schema Generator", () => {
  test("generates imports", () => {
    const imports = generator.generateImports();
    expect(imports).toContain("Type");
    expect(imports).toContain("Static");
  });

  test("generates SelectUser schema with all columns", () => {
    const schema = generator.generateSelectSchema(usersTable, ir);
    expect(schema).toContain("SelectUser");
    expect(schema).toContain("Type.Number()");
    expect(schema).toContain("Type.String()");
    expect(schema).toContain("Type.Null()");
    expect(schema).toContain("Type.Date()");
  });

  test("generates InsertUser schema omitting columns with defaults", () => {
    const schema = generator.generateInsertSchema(usersTable, ir);
    expect(schema).toContain("InsertUser");
    expect(schema).not.toContain('"id"');
    expect(schema).not.toContain('"created_at"');
    expect(schema).toContain('"name"');
    expect(schema).toContain('"email"');
  });

  test("nullable columns use Union with Null in Select", () => {
    const schema = generator.generateSelectSchema(usersTable, ir);
    expect(schema).toContain("Type.Union([Type.String(), Type.Null()])");
  });

  test("nullable columns without default are Optional in Insert", () => {
    const schema = generator.generateInsertSchema(usersTable, ir);
    expect(schema).toContain("Type.Optional(");
  });

  test("array columns use Type.Array()", () => {
    const schema = generator.generateSelectSchema(usersTable, ir);
    expect(schema).toContain("Type.Array(Type.String())");
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
