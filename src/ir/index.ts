export type SqlTypeCategory =
  | "string"
  | "number"
  | "boolean"
  | "date"
  | "json"
  | "uuid"
  | "binary"
  | "enum"
  | "unknown";

export type JsonShape =
  | { kind: "string" }
  | { kind: "number" }
  | { kind: "boolean" }
  | { kind: "object"; fields: Record<string, JsonShape> }
  | { kind: "array"; element: JsonShape }
  | { kind: "nullable"; inner: JsonShape };

export interface SqlType {
  raw: string;
  normalized: string;
  category: SqlTypeCategory;
  elementType?: SqlType;
  enumName?: string;
  enumValues?: string[];
  jsonShape?: JsonShape;
}

export interface ColumnDef {
  name: string;
  alias?: string;
  sourceTable?: string;
  type: SqlType;
  nullable: boolean;
  hasDefault: boolean;
}

export interface TableDef {
  name: string;
  columns: ColumnDef[];
  primaryKey: string[];
  uniqueConstraints: string[][];
}

export type QueryCommand = "one" | "many" | "exec" | "execresult";

export interface ParamDef {
  index: number;
  name: string;
  type: SqlType;
}

export interface QueryDef {
  name: string;
  command: QueryCommand;
  sql: string;
  params: ParamDef[];
  returns: ColumnDef[];
  sourceFile: string;
}

export interface EnumDef {
  name: string;
  values: string[];
}

export interface SqlcxIR {
  tables: TableDef[];
  queries: QueryDef[];
  enums: EnumDef[];
}
