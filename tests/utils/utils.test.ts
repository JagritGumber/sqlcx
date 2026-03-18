import { describe, expect, test } from "bun:test";
import { pascalCase, camelCase } from "@/utils";

describe("pascalCase", () => {
  test("snake_case", () => expect(pascalCase("user_profile")).toBe("UserProfile"));
  test("kebab-case", () => expect(pascalCase("user-profile")).toBe("UserProfile"));
  test("single word", () => expect(pascalCase("users")).toBe("Users"));
});

describe("camelCase", () => {
  test("snake_case", () => expect(camelCase("user_profile")).toBe("userProfile"));
  test("single word", () => expect(camelCase("users")).toBe("users"));
});
