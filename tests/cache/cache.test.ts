import { describe, expect, test, beforeEach, afterEach } from "bun:test";
import { computeHash, readCache, writeCache } from "@/cache";
import type { SqlcxIR } from "@/ir";
import { mkdirSync, rmSync, existsSync } from "fs";
import { join } from "path";

const testCacheDir = join(import.meta.dir, ".test-sqlcx");

beforeEach(() => {
  if (existsSync(testCacheDir)) rmSync(testCacheDir, { recursive: true });
});

afterEach(() => {
  if (existsSync(testCacheDir)) rmSync(testCacheDir, { recursive: true });
});

describe("IR Cache", () => {
  test("computeHash produces consistent SHA-256", () => {
    const files = [{ path: "a.sql", content: "CREATE TABLE a ();" }];
    const hash1 = computeHash(files);
    const hash2 = computeHash(files);
    expect(hash1).toBe(hash2);
    expect(hash1).toHaveLength(64);
  });

  test("computeHash changes when content changes", () => {
    const hash1 = computeHash([{ path: "a.sql", content: "v1" }]);
    const hash2 = computeHash([{ path: "a.sql", content: "v2" }]);
    expect(hash1).not.toBe(hash2);
  });

  test("computeHash is order-independent", () => {
    const hash1 = computeHash([
      { path: "b.sql", content: "B" },
      { path: "a.sql", content: "A" },
    ]);
    const hash2 = computeHash([
      { path: "a.sql", content: "A" },
      { path: "b.sql", content: "B" },
    ]);
    expect(hash1).toBe(hash2);
  });

  test("write and read cache round-trip", () => {
    const ir: SqlcxIR = { tables: [], queries: [], enums: [] };
    writeCache(testCacheDir, ir, "abc123");
    const result = readCache(testCacheDir, "abc123");
    expect(result).not.toBeNull();
    expect(result!.tables).toEqual([]);
  });

  test("read returns null on hash mismatch", () => {
    const ir: SqlcxIR = { tables: [], queries: [], enums: [] };
    writeCache(testCacheDir, ir, "abc123");
    const result = readCache(testCacheDir, "different");
    expect(result).toBeNull();
  });

  test("read returns null when cache doesn't exist", () => {
    const result = readCache("/tmp/nonexistent-sqlcx-cache", "abc123");
    expect(result).toBeNull();
  });

  test("write creates nested directories", () => {
    const ir: SqlcxIR = { tables: [], queries: [], enums: [] };
    const nested = join(testCacheDir, "nested", "deep");
    writeCache(nested, ir, "abc123");
    expect(existsSync(join(nested, "ir.json"))).toBe(true);
  });
});
