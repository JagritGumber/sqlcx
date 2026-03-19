import { createHash } from "crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync, renameSync } from "fs";
import { join } from "path";
import type { SqlcxIR } from "@/ir";

interface CacheFile {
  hash: string;
  ir: SqlcxIR;
}

export function computeHash(files: { path: string; content: string }[]): string {
  const sorted = [...files].sort((a, b) => a.path.localeCompare(b.path));
  // Include both path and content in hash, with null byte separators
  // to avoid collisions from file splits/merges/renames
  const hash = createHash("sha256");
  for (const f of sorted) {
    hash.update(f.path);
    hash.update("\0");
    hash.update(f.content);
    hash.update("\0");
  }
  return hash.digest("hex");
}

export function writeCache(cacheDir: string, ir: SqlcxIR, hash: string): void {
  if (!existsSync(cacheDir)) mkdirSync(cacheDir, { recursive: true });
  const data: CacheFile = { hash, ir };
  const cachePath = join(cacheDir, "ir.json");
  const tempPath = cachePath + ".tmp";
  // Write to temp file then atomic rename — safe against interruptions
  writeFileSync(tempPath, JSON.stringify(data, null, 2));
  renameSync(tempPath, cachePath);
}

export function readCache(cacheDir: string, expectedHash: string): SqlcxIR | null {
  const cachePath = join(cacheDir, "ir.json");
  if (!existsSync(cachePath)) return null;
  try {
    const data: CacheFile = JSON.parse(readFileSync(cachePath, "utf-8"));
    if (data.hash !== expectedHash) return null;
    return data.ir;
  } catch {
    // Corrupted cache file — treat as cache miss
    return null;
  }
}
