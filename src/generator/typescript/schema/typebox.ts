import type { SchemaGenerator } from "@/generator/interface";
import type { SqlcxIR, TableDef, EnumDef, SqlType, ColumnDef } from "@/ir";
import { pascalCase } from "@/utils";

function escapeString(str: string): string {
  return str.replace(/\\/g, "\\\\").replace(/"/g, '\\"').replace(/\n/g, "\\n");
}

function typeBoxType(type: SqlType, ir: SqlcxIR): string {
  if (type.elementType) {
    return `Type.Array(${typeBoxType(type.elementType, ir)})`;
  }

  switch (type.category) {
    case "string":
      return "Type.String()";
    case "number":
      return "Type.Number()";
    case "boolean":
      return "Type.Boolean()";
    case "date":
      return "Type.Date()";
    case "json":
      return "Type.Any()";
    case "uuid":
      return "Type.String()";
    case "binary":
      return "Type.Uint8Array()";
    case "enum": {
      // Reference the named enum schema variable instead of inlining
      if (type.enumName) {
        return pascalCase(type.enumName);
      }
      return "Type.String()";
    }
    case "unknown":
      return "Type.Unknown()";
  }
}

function selectColumn(col: ColumnDef, ir: SqlcxIR): string {
  const base = typeBoxType(col.type, ir);
  if (col.nullable) {
    return `Type.Union([${base}, Type.Null()])`;
  }
  return base;
}

function insertColumn(col: ColumnDef, ir: SqlcxIR): string {
  const base = typeBoxType(col.type, ir);
  if (col.hasDefault) {
    // Columns with defaults are optional in inserts (user can override or omit)
    if (col.nullable) {
      return `Type.Optional(Type.Union([${base}, Type.Null()]))`;
    }
    return `Type.Optional(${base})`;
  }
  if (col.nullable) {
    return `Type.Optional(Type.Union([${base}, Type.Null()]))`;
  }
  return base;
}

function objectBody(
  columns: ColumnDef[],
  mapper: (col: ColumnDef, ir: SqlcxIR) => string,
  ir: SqlcxIR,
): string {
  const fields = columns
    .map((col) => `  "${escapeString(col.name)}": ${mapper(col, ir)}`)
    .join(",\n");
  return `{\n${fields}\n}`;
}

export function createTypeBoxGenerator(): SchemaGenerator {
  return {
    name: "typebox",

    generateImports(): string {
      return `import { Type, type Static } from "@sinclair/typebox";\n\ntype Prettify<T> = { [K in keyof T]: T[K] } & {};`;
    },

    generateEnumSchema(enumDef: EnumDef): string {
      const name = pascalCase(enumDef.name);
      const literals = enumDef.values
        .map((v) => `Type.Literal("${escapeString(v)}")`)
        .join(", ");
      return `export const ${name} = Type.Union([${literals}]);`;
    },

    generateSelectSchema(table: TableDef, ir: SqlcxIR): string {
      const name = `Select${pascalCase(table.name)}`;
      const body = objectBody(table.columns, selectColumn, ir);
      return `export const ${name} = Type.Object(${body});`;
    },

    generateInsertSchema(table: TableDef, ir: SqlcxIR): string {
      const name = `Insert${pascalCase(table.name)}`;
      // Include ALL columns — those with defaults are wrapped in Type.Optional()
      const body = objectBody(table.columns, insertColumn, ir);
      return `export const ${name} = Type.Object(${body});`;
    },

    generateTypeAlias(name: string, schemaVarName: string): string {
      return `export type ${name} = Prettify<Static<typeof ${schemaVarName}>>;`;
    },
  };
}
