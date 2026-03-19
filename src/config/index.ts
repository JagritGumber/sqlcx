import type { DatabaseParser } from "@/parser/interface";
import type { LanguagePlugin } from "@/generator/interface";

export interface SqlcxConfig {
  sql: string;
  parser: DatabaseParser;
  targets: LanguagePlugin[];
  overrides?: Record<string, string>;
}

export function defineConfig(config: SqlcxConfig): SqlcxConfig {
  return config;
}

export async function loadConfig(configPath: string): Promise<SqlcxConfig> {
  const resolved = Bun.resolveSync(configPath, process.cwd());
  const mod = await import(resolved);
  return mod.default as SqlcxConfig;
}
