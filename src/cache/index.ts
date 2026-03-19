import { createHash } from "crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { join } from "path";
import type { SqlcxIR } from "@/ir";

interface CacheFile {
  hash: string;
  ir: SqlcxIR;
}

export function computeHash(files: { path: string; content: string }[]): string {
  const sorted = [...files].sort((a, b) => a.path.localeCompare(b.path));
  const combined = sorted.map((f) => f.content).join("\n");
  return createHash("sha256").update(combined).digest("hex");
}

export function writeCache(cacheDir: string, ir: SqlcxIR, hash: string): void {
  if (!existsSync(cacheDir)) mkdirSync(cacheDir, { recursive: true });
  const data: CacheFile = { hash, ir };
  writeFileSync(join(cacheDir, "ir.json"), JSON.stringify(data, null, 2));
}

export function readCache(cacheDir: string, expectedHash: string): SqlcxIR | null {
  const cachePath = join(cacheDir, "ir.json");
  if (!existsSync(cachePath)) return null;
  const data: CacheFile = JSON.parse(readFileSync(cachePath, "utf-8"));
  if (data.hash !== expectedHash) return null;
  return data.ir;
}
