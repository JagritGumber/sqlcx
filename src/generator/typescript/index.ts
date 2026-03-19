import type {
  LanguagePlugin,
  SchemaGenerator,
  DriverGenerator,
  GeneratedFile,
  LanguageOptions,
} from "@/generator/interface";
import type { SqlcxIR } from "@/ir";
import { pascalCase } from "@/utils";
import path from "node:path";

function joinPath(base: string, filename: string): string {
  const joined = path.join(base, filename);
  // Preserve leading "./" if the base had it
  if (base.startsWith("./") && !joined.startsWith("./")) {
    return "./" + joined;
  }
  return joined;
}

interface TypeScriptPluginOptions {
  schema: SchemaGenerator;
  driver: DriverGenerator;
}

function generateSchemaFile(
  schema: SchemaGenerator,
  ir: SqlcxIR,
): string {
  const parts: string[] = [];

  parts.push(schema.generateImports());

  for (const enumDef of ir.enums) {
    parts.push(schema.generateEnumSchema(enumDef));
  }

  for (const table of ir.tables) {
    parts.push(schema.generateSelectSchema(table, ir));
    parts.push(schema.generateInsertSchema(table, ir));
  }

  for (const table of ir.tables) {
    const selectName = `Select${pascalCase(table.name)}`;
    const insertName = `Insert${pascalCase(table.name)}`;
    parts.push(schema.generateTypeAlias(selectName, selectName));
    parts.push(schema.generateTypeAlias(insertName, insertName));
  }

  for (const enumDef of ir.enums) {
    const name = pascalCase(enumDef.name);
    parts.push(schema.generateTypeAlias(name, name));
  }

  return parts.join("\n\n") + "\n";
}

const DATABASE_CLIENT_INTERFACE = `export interface DatabaseClient {
  query<T>(sql: string, params: unknown[]): Promise<T[]>;
  queryOne<T>(sql: string, params: unknown[]): Promise<T | null>;
  execute(sql: string, params: unknown[]): Promise<{ rowsAffected: number }>;
}`;

function generateClientFile(driver: DriverGenerator): string {
  const parts: string[] = [];

  const driverImports = driver.generateImports();
  if (driverImports) {
    parts.push(driverImports);
  }

  parts.push(DATABASE_CLIENT_INTERFACE);
  parts.push(driver.generateClientAdapter());

  return parts.join("\n\n") + "\n";
}

function generateQueryFiles(
  driver: DriverGenerator,
  ir: SqlcxIR,
  outDir: string,
): GeneratedFile[] {
  const grouped = new Map<string, typeof ir.queries>();

  for (const query of ir.queries) {
    const existing = grouped.get(query.sourceFile);
    if (existing) {
      existing.push(query);
    } else {
      grouped.set(query.sourceFile, [query]);
    }
  }

  const files: GeneratedFile[] = [];

  for (const [sourceFile, queries] of grouped) {
    const basename = path.basename(sourceFile, path.extname(sourceFile));
    const filename = `${basename}.queries.ts`;

    const parts: string[] = [];
    parts.push(`import type { DatabaseClient } from "./client";`);

    for (const query of queries) {
      parts.push(driver.generateQueryFunction(query));
    }

    files.push({
      path: joinPath(outDir, filename),
      content: parts.join("\n\n") + "\n",
    });
  }

  return files;
}

export function createTypeScriptPlugin(
  options: TypeScriptPluginOptions,
): LanguagePlugin {
  const { schema, driver } = options;

  return {
    language: "typescript",
    fileExtension: ".ts",

    generate(ir: SqlcxIR, langOptions: LanguageOptions): GeneratedFile[] {
      const outDir = langOptions.out;
      const files: GeneratedFile[] = [];

      files.push({
        path: joinPath(outDir, "schema.ts"),
        content: generateSchemaFile(schema, ir),
      });

      files.push({
        path: joinPath(outDir, "client.ts"),
        content: generateClientFile(driver),
      });

      files.push(...generateQueryFiles(driver, ir, outDir));

      return files;
    },
  };
}
