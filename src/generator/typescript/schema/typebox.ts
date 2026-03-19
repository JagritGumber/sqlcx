import type { SchemaGenerator } from "@/generator/interface";
import type { SqlcxIR, TableDef, EnumDef, SqlType, ColumnDef, JsonShape } from "@/ir";
import { pascalCase } from "@/utils";

/** Safely escape a string for embedding in generated JS/TS double-quoted literals */
function escapeString(str: string): string {
  return JSON.stringify(str).slice(1, -1); // strip outer quotes from JSON.stringify
}

function jsonShapeToTypeBox(shape: JsonShape): string {
  switch (shape.kind) {
    case "string":
      return "Type.String()";
    case "number":
      return "Type.Number()";
    case "boolean":
      return "Type.Boolean()";
    case "object": {
      const fields = Object.entries(shape.fields)
        .map(([key, val]) => `"${escapeString(key)}": ${jsonShapeToTypeBox(val)}`)
        .join(", ");
      return `Type.Object({ ${fields} })`;
    }
    case "array":
      return `Type.Array(${jsonShapeToTypeBox(shape.element)})`;
    case "nullable":
      return `Type.Union([${jsonShapeToTypeBox(shape.inner)}, Type.Null()])`;
  }
}

function typeBoxType(type: SqlType): string {
  // Inline @enum annotation takes precedence
  if (type.enumValues) {
    const literals = type.enumValues
      .map((v) => `Type.Literal("${escapeString(v)}")`)
      .join(", ");
    return `Type.Union([${literals}])`;
  }

  // Inline @json annotation takes precedence over generic json category
  if (type.jsonShape) {
    return jsonShapeToTypeBox(type.jsonShape);
  }

  if (type.elementType) {
    return `Type.Array(${typeBoxType(type.elementType)})`;
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
      if (type.enumName) {
        return pascalCase(type.enumName);
      }
      return "Type.String()";
    }
    case "unknown":
      return "Type.Unknown()";
    default: {
      const _exhaustive: never = type.category;
      return _exhaustive;
    }
  }
}

function selectColumn(col: ColumnDef): string {
  const base = typeBoxType(col.type);
  if (col.nullable) {
    return `Type.Union([${base}, Type.Null()])`;
  }
  return base;
}

function insertColumn(col: ColumnDef): string {
  const base = typeBoxType(col.type);
  if (col.hasDefault) {
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
  mapper: (col: ColumnDef) => string,
): string {
  const fields = columns
    .map((col) => `  "${escapeString(col.name)}": ${mapper(col)}`)
    .join(",\n");
  return `{\n${fields}\n}`;
}

export function createTypeBoxGenerator(): SchemaGenerator {
  return {
    name: "typebox",

    generateImports(): string {
      return `import { Type, type Static } from "@sinclair/typebox";\n\n// Requires @sinclair/typebox >= 0.31.0 (for Type.Date and Type.Uint8Array)\n\ntype Prettify<T> = { [K in keyof T]: T[K] } & {};`;
    },

    generateEnumSchema(enumDef: EnumDef): string {
      const name = pascalCase(enumDef.name);
      const literals = enumDef.values
        .map((v) => `Type.Literal("${escapeString(v)}")`)
        .join(", ");
      return `export const ${name} = Type.Union([${literals}]);`;
    },

    generateSelectSchema(table: TableDef, _ir: SqlcxIR): string {
      const name = `Select${pascalCase(table.name)}`;
      const body = objectBody(table.columns, selectColumn);
      return `export const ${name} = Type.Object(${body});`;
    },

    generateInsertSchema(table: TableDef, _ir: SqlcxIR): string {
      const name = `Insert${pascalCase(table.name)}`;
      const body = objectBody(table.columns, insertColumn);
      return `export const ${name} = Type.Object(${body});`;
    },

    generateTypeAlias(name: string, schemaVarName: string): string {
      return `export type ${name} = Prettify<Static<typeof ${schemaVarName}>>;`;
    },
  };
}
