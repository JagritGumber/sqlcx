import starlightPlugin from '@astrojs/starlight-tailwind';

const accent = { 200: '#e8f0a0', 600: '#7a8a20', 900: '#2a3000', 950: '#1d2200' };
const gray = { 100: '#f5f5f5', 200: '#e5e5e5', 300: '#b0b0b0', 400: '#888888', 500: '#666666', 700: '#333333', 800: '#1a1a1a', 900: '#111111' };

export default {
  content: ['./src/**/*.{astro,html,js,jsx,md,mdx,svelte,ts,tsx,vue}'],
  theme: { extend: { colors: { accent, gray } } },
  plugins: [starlightPlugin()],
};
