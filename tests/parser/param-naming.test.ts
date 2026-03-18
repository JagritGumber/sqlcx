import { describe, expect, test } from "bun:test";
import { resolveParamNames } from "@/parser/param-naming";

describe("resolveParamNames", () => {
  test("simple unique columns", () => {
    const result = resolveParamNames([
      { index: 1, column: "id" },
      { index: 2, column: "name" },
    ]);
    expect(result).toEqual(["id", "name"]);
  });

  test("collision renames both to _1 and _2", () => {
    const result = resolveParamNames([
      { index: 1, column: "created_at" },
      { index: 2, column: "created_at" },
    ]);
    expect(result).toEqual(["created_at_1", "created_at_2"]);
  });

  test("null column falls back to param_N", () => {
    const result = resolveParamNames([
      { index: 1, column: null },
    ]);
    expect(result).toEqual(["param_1"]);
  });

  test("annotation override takes precedence", () => {
    const result = resolveParamNames([
      { index: 1, column: "created_at", override: "start_date" },
      { index: 2, column: "created_at", override: "end_date" },
    ]);
    expect(result).toEqual(["start_date", "end_date"]);
  });

  test("expression extraction — column from LOWER(name)", () => {
    const result = resolveParamNames([
      { index: 1, column: "name" },
    ]);
    expect(result).toEqual(["name"]);
  });

  test("mixed: some overrides, some inferred, some collisions", () => {
    const result = resolveParamNames([
      { index: 1, column: "status" },
      { index: 2, column: "status" },
      { index: 3, column: null, override: "limit" },
    ]);
    expect(result).toEqual(["status_1", "status_2", "limit"]);
  });
});
