import { describe, expect, test } from "bun:test";
import { createTypeBoxGenerator } from "@/generator/typescript/schema/typebox";
import type { SqlcxIR, TableDef, SqlType, JsonShape } from "@/ir";

const generator = createTypeBoxGenerator();
const emptyIR: SqlcxIR = { tables: [], queries: [], enums: [] };

function makeTable(colType: SqlType, nullable = false): TableDef {
  return {
    name: "test",
    columns: [
      {
        name: "col",
        type: colType,
        nullable,
        hasDefault: false,
      },
    ],
    primaryKey: [],
    uniqueConstraints: [],
  };
}

describe("TypeBox Annotations", () => {
  test("enumValues generates Type.Union of Type.Literal values", () => {
    const table = makeTable({
      raw: "TEXT",
      normalized: "text",
      category: "enum",
      enumValues: ["admin", "user", "guest"],
    });
    const schema = generator.generateSelectSchema(table, emptyIR);
    expect(schema).toContain(
      'Type.Union([Type.Literal("admin"), Type.Literal("user"), Type.Literal("guest")])',
    );
  });

  test("jsonShape object generates Type.Object with correct fields", () => {
    const table = makeTable({
      raw: "JSONB",
      normalized: "jsonb",
      category: "json",
      jsonShape: {
        kind: "object",
        fields: {
          theme: { kind: "string" },
          lang: { kind: "string" },
        },
      },
    });
    const schema = generator.generateSelectSchema(table, emptyIR);
    expect(schema).toContain("Type.Object({");
    expect(schema).toContain('"theme": Type.String()');
    expect(schema).toContain('"lang": Type.String()');
  });

  test("jsonShape array generates Type.Array", () => {
    const table = makeTable({
      raw: "JSONB",
      normalized: "jsonb",
      category: "json",
      jsonShape: { kind: "array", element: { kind: "string" } },
    });
    const schema = generator.generateSelectSchema(table, emptyIR);
    expect(schema).toContain("Type.Array(Type.String())");
  });

  test("jsonShape nullable generates Type.Union with Type.Null", () => {
    const table = makeTable({
      raw: "JSONB",
      normalized: "jsonb",
      category: "json",
      jsonShape: { kind: "nullable", inner: { kind: "number" } },
    });
    const schema = generator.generateSelectSchema(table, emptyIR);
    expect(schema).toContain("Type.Union([Type.Number(), Type.Null()])");
  });

  test("jsonShape nested object generates nested Type.Object", () => {
    const table = makeTable({
      raw: "JSONB",
      normalized: "jsonb",
      category: "json",
      jsonShape: {
        kind: "object",
        fields: {
          settings: {
            kind: "object",
            fields: {
              dark_mode: { kind: "boolean" },
            },
          },
        },
      },
    });
    const schema = generator.generateSelectSchema(table, emptyIR);
    expect(schema).toContain(
      'Type.Object({ "settings": Type.Object({ "dark_mode": Type.Boolean() }) })',
    );
  });

  test("nullable JSONB column with jsonShape wraps whole thing in Union with Null", () => {
    const table = makeTable(
      {
        raw: "JSONB",
        normalized: "jsonb",
        category: "json",
        jsonShape: {
          kind: "object",
          fields: {
            theme: { kind: "string" },
          },
        },
      },
      true, // nullable
    );
    const schema = generator.generateSelectSchema(table, emptyIR);
    // The selectColumn function wraps nullable columns in Type.Union([base, Type.Null()])
    expect(schema).toContain('Type.Union([Type.Object({ "theme": Type.String() }), Type.Null()])');
  });
});
