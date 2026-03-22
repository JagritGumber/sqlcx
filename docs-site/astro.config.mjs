import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  vite: {
    plugins: [tailwindcss()],
  },
  integrations: [
    starlight({
      title: 'sqlcx',
      logo: { src: './src/assets/banner.png', replacesTitle: true },
      favicon: '/icon.png',
      social: {
        github: 'https://github.com/JagritGumber/sqlcx',
      },
      customCss: ['./src/styles/custom.css'],
      sidebar: [
        {
          label: 'Getting Started',
          items: [
            { label: 'Why sqlcx?', slug: 'getting-started/why-sqlcx' },
            { label: 'Installation', slug: 'getting-started/installation' },
            { label: 'Quick Start', slug: 'getting-started/quick-start' },
            { label: 'Configuration', slug: 'getting-started/configuration' },
          ],
        },
        {
          label: 'Databases',
          items: [
            { label: 'PostgreSQL', slug: 'databases/postgresql' },
            { label: 'MySQL', slug: 'databases/mysql' },
            { label: 'SQLite', slug: 'databases/sqlite' },
          ],
        },
        {
          label: 'TypeScript',
          items: [
            { label: 'TypeBox', slug: 'typescript/typebox' },
            { label: 'Zod', slug: 'typescript/zod' },
            { label: 'Bun SQL', slug: 'typescript/bun-sql' },
            { label: 'pg', slug: 'typescript/pg' },
          ],
        },
        {
          label: 'Go',
          items: [
            { label: 'Structs', slug: 'go/structs' },
            { label: 'database/sql', slug: 'go/database-sql' },
          ],
        },
        {
          label: 'Rust',
          items: [
            { label: 'Serde', slug: 'rust/serde' },
            { label: 'sqlx', slug: 'rust/sqlx' },
          ],
        },
        {
          label: 'SQL Guide',
          items: [
            { label: 'Query Annotations', slug: 'sql-guide/query-annotations' },
            { label: 'Annotations', slug: 'sql-guide/annotations' },
            { label: 'Input & Output Types', slug: 'sql-guide/input-output-types' },
            { label: 'SELECT Patterns', slug: 'sql-guide/select-patterns' },
          ],
        },
        {
          label: 'CLI Reference',
          items: [
            { label: 'generate', slug: 'cli/generate' },
            { label: 'check', slug: 'cli/check' },
            { label: 'init', slug: 'cli/init' },
            { label: 'schema', slug: 'cli/schema' },
          ],
        },
        {
          label: 'Advanced',
          items: [
            { label: 'IR Format', slug: 'advanced/ir-format' },
            { label: 'Caching', slug: 'advanced/caching' },
            { label: 'Plugin System', slug: 'advanced/plugin-system' },
            { label: 'Community Plugins', slug: 'advanced/community-plugins' },
          ],
        },
        {
          label: 'Coming Soon',
          items: [
            { label: 'Python', slug: 'coming-soon/python' },
            { label: 'Migrations', slug: 'coming-soon/migrations' },
            { label: 'Watch Mode', slug: 'coming-soon/watch-mode' },
            { label: 'DSL Compiler', slug: 'coming-soon/dsl-compiler' },
          ],
        },
        {
          label: 'Comparison',
          items: [
            { label: 'Comparison', slug: 'comparison' },
          ],
        },
      ],
    }),
  ],
});
