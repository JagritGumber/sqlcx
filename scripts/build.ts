// Build CLI as a single executable entry point
const cli = await Bun.build({
  entrypoints: ["src/cli/index.ts"],
  outdir: "dist",
  target: "bun",
  format: "esm",
  naming: "[dir]/cli.js",
});

if (!cli.success) {
  console.error("CLI build failed:");
  for (const log of cli.logs) console.error(log);
  process.exit(1);
}

// Build library entry points
const lib = await Bun.build({
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

if (!lib.success) {
  console.error("Library build failed:");
  for (const log of lib.logs) console.error(log);
  process.exit(1);
}

console.log("Build complete → dist/");
