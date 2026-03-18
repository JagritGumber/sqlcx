import type { DriverGenerator } from "@/generator/interface";
import type { QueryDef, SqlType } from "@/ir";
import { camelCase, pascalCase } from "@/utils";

/** Split PascalCase/camelCase into words before applying case utils */
function splitWords(str: string): string {
  return str.replace(/([a-z])([A-Z])/g, "$1_$2");
}

function toCamel(str: string): string {
  return camelCase(splitWords(str));
}

function toPascal(str: string): string {
  return pascalCase(splitWords(str));
}

function tsType(type: SqlType): string {
  // Handle arrays
  if (type.elementType) {
    return `${tsType(type.elementType)}[]`;
  }

  switch (type.category) {
    case "string":
    case "uuid":
    case "enum":
      return "string";
    case "number":
      return "number";
    case "boolean":
      return "boolean";
    case "date":
      return "Date";
    case "json":
      return "unknown";
    case "binary":
      return "Uint8Array";
    case "unknown":
      return "unknown";
  }
}

function generateRowType(query: QueryDef): string {
  if (query.returns.length === 0) return "";
  const typeName = `${toPascal(query.name)}Row`;
  const fields = query.returns
    .map((col) => {
      const fieldName = col.alias ?? col.name;
      const type = tsType(col.type);
      const nullable = col.nullable ? " | null" : "";
      return `  ${fieldName}: ${type}${nullable};`;
    })
    .join("\n");
  return `export interface ${typeName} {\n${fields}\n}`;
}

function generateParamsType(query: QueryDef): string {
  if (query.params.length === 0) return "";
  const typeName = `${toPascal(query.name)}Params`;
  const fields = query.params
    .map((p) => `  ${p.name}: ${tsType(p.type)};`)
    .join("\n");
  return `export interface ${typeName} {\n${fields}\n}`;
}

export function createBunSqlGenerator(): DriverGenerator {
  return {
    name: "bun-sql",

    generateImports(): string {
      return "";
    },

    generateClientAdapter(): string {
      return `interface BunSqlDriver {
  unsafe(query: string, values?: unknown[]): Promise<any[] & { count: number }>;
}

export class BunSqlClient implements DatabaseClient {
  private sql: BunSqlDriver;

  constructor(sql: BunSqlDriver) {
    this.sql = sql;
  }

  async query<T>(text: string, values?: unknown[]): Promise<T[]> {
    const result = await this.sql.unsafe(text, values);
    return [...result] as T[];
  }

  async queryOne<T>(text: string, values?: unknown[]): Promise<T | null> {
    const rows = await this.query<T>(text, values);
    return rows[0] ?? null;
  }

  async execute(text: string, values?: unknown[]): Promise<{ rowsAffected: number }> {
    const result = await this.sql.unsafe(text, values);
    return { rowsAffected: result.count };
  }
}`;
    },

    generateQueryFunction(query: QueryDef): string {
      const fnName = toCamel(query.name);
      const rowType = generateRowType(query);
      const hasParams = query.params.length > 0;
      const paramsInterface = generateParamsType(query);
      const paramsTypeName = `${toPascal(query.name)}Params`;
      // JSON.stringify handles newlines, quotes, backslashes safely
      const sqlConst = `export const ${fnName}Sql = ${JSON.stringify(query.sql)};`;

      const paramsSig = hasParams ? `, params: ${paramsTypeName}` : "";
      const valuesArg = hasParams
        ? `[${query.params.map((p) => `params.${p.name}`).join(", ")}]`
        : "[]";

      let returnType: string;
      let body: string;

      switch (query.command) {
        case "one": {
          const typeName = `${toPascal(query.name)}Row`;
          returnType = `Promise<${typeName} | null>`;
          body = `  return client.queryOne<${typeName}>(${fnName}Sql, ${valuesArg});`;
          break;
        }
        case "many": {
          const typeName = `${toPascal(query.name)}Row`;
          returnType = `Promise<${typeName}[]>`;
          body = `  return client.query<${typeName}>(${fnName}Sql, ${valuesArg});`;
          break;
        }
        case "exec": {
          returnType = "Promise<void>";
          body = `  await client.execute(${fnName}Sql, ${valuesArg});`;
          break;
        }
        case "execresult": {
          returnType = "Promise<{ rowsAffected: number }>";
          body = `  return client.execute(${fnName}Sql, ${valuesArg});`;
          break;
        }
      }

      const parts: string[] = [];
      if (rowType) parts.push(rowType);
      if (paramsInterface) parts.push(paramsInterface);
      parts.push(sqlConst);
      parts.push(
        `export async function ${fnName}(client: DatabaseClient${paramsSig}): ${returnType} {\n${body}\n}`
      );

      return parts.join("\n\n");
    },
  };
}
