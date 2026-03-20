import { readFileSync, writeFileSync, mkdirSync, cpSync } from "fs";
import { join } from "path";

// Build CLI as a single executable entry point
await Bun.build({
  entrypoints: ["src/cli/index.ts"],
  outdir: "dist",
  target: "bun",
  format: "esm",
  naming: "[dir]/cli.js",
});

// Build library entry points
await Bun.build({
  entrypoints: [
    "src/index.ts",
    "src/config/index.ts",
    "src/parser/postgres.ts",
    "src/generator/typescript/index.ts",
    "src/generator/typescript/schema/typebox.ts",
    "src/generator/typescript/driver/bun-sql.ts",
  ],
  outdir: "dist",
  target: "bun",
  format: "esm",
  splitting: true,
});

console.log("Build complete → dist/");
