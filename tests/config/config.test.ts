import { describe, expect, test } from "bun:test";
import { defineConfig, type SqlcxConfig } from "@/config";

describe("defineConfig", () => {
  test("returns config as-is (type helper)", () => {
    const config = defineConfig({
      sql: "./sql",
      parser: { dialect: "postgres", parseSchema: () => [], parseQueries: () => [], parseEnums: () => [] },
      targets: [],
    });
    expect(config.sql).toBe("./sql");
  });

  test("overrides are optional", () => {
    const config = defineConfig({
      sql: "./sql",
      parser: { dialect: "postgres", parseSchema: () => [], parseQueries: () => [], parseEnums: () => [] },
      targets: [],
    });
    expect(config.overrides).toBeUndefined();
  });

  test("preserves parser and targets", () => {
    const mockParser = { dialect: "postgres", parseSchema: () => [], parseQueries: () => [], parseEnums: () => [] };
    const config = defineConfig({
      sql: "./sql",
      parser: mockParser,
      targets: [],
      overrides: { "uuid": "string" },
    });
    expect(config.parser.dialect).toBe("postgres");
    expect(config.targets).toEqual([]);
    expect(config.overrides).toEqual({ "uuid": "string" });
  });
});
