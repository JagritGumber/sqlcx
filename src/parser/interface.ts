import type { TableDef, QueryDef, EnumDef } from "@/ir";

export interface DatabaseParser {
  dialect: string;
  parseSchema(sql: string): TableDef[];
  parseQueries(sql: string, tables: TableDef[]): QueryDef[];
  parseEnums(sql: string): EnumDef[];
}
