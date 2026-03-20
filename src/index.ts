// Public API
export { defineConfig } from "@/config";
export type { SqlcxConfig } from "@/config";
export type {
  SqlcxIR,
  TableDef,
  ColumnDef,
  QueryDef,
  ParamDef,
  EnumDef,
  SqlType,
  SqlTypeCategory,
  QueryCommand,
  JsonShape,
} from "@/ir";
export type { DatabaseParser } from "@/parser/interface";
export type {
  LanguagePlugin,
  SchemaGenerator,
  DriverGenerator,
  GeneratedFile,
  LanguageOptions,
} from "@/generator/interface";
