import { describe, expect, test } from "bun:test";
import type { SqlcxIR, SqlType } from "@/ir";

describe("IR types", () => {
  test("SqlcxIR is JSON-serializable", () => {
    const ir: SqlcxIR = {
      tables: [
        {
          name: "users",
          columns: [
            {
              name: "id",
              type: { raw: "SERIAL", normalized: "serial", category: "number" },
              nullable: false,
              hasDefault: true,
            },
            {
              name: "name",
              type: { raw: "TEXT", normalized: "text", category: "string" },
              nullable: false,
              hasDefault: false,
            },
          ],
          primaryKey: ["id"],
          uniqueConstraints: [],
        },
      ],
      queries: [
        {
          name: "GetUser",
          command: "one",
          sql: "SELECT * FROM users WHERE id = $1",
          params: [
            {
              index: 1,
              name: "id",
              type: { raw: "SERIAL", normalized: "serial", category: "number" },
            },
          ],
          returns: [
            {
              name: "id",
              type: { raw: "SERIAL", normalized: "serial", category: "number" },
              nullable: false,
              hasDefault: true,
            },
            {
              name: "name",
              type: { raw: "TEXT", normalized: "text", category: "string" },
              nullable: false,
              hasDefault: false,
            },
          ],
          sourceFile: "queries/users.sql",
        },
      ],
      enums: [],
    };

    const json = JSON.stringify(ir);
    const parsed = JSON.parse(json) as SqlcxIR;
    expect(parsed.tables).toHaveLength(1);
    expect(parsed.tables[0].name).toBe("users");
    expect(parsed.queries).toHaveLength(1);
    expect(parsed.queries[0].name).toBe("GetUser");
  });

  test("SqlType with elementType for arrays", () => {
    const arrayType: SqlType = {
      raw: "TEXT[]",
      normalized: "text[]",
      category: "string",
      elementType: { raw: "TEXT", normalized: "text", category: "string" },
    };
    const json = JSON.stringify(arrayType);
    const parsed = JSON.parse(json) as SqlType;
    expect(parsed.elementType?.category).toBe("string");
  });

  test("SqlType with enumName", () => {
    const enumType: SqlType = {
      raw: "user_status",
      normalized: "user_status",
      category: "enum",
      enumName: "user_status",
    };
    expect(enumType.enumName).toBe("user_status");
  });
});
