import { svelte } from '@sveltejs/vite-plugin-svelte';
import { mogPlugin } from 'vite-plugin-mog';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [
    mogPlugin({
      mode: 'svelte',
      theme: { light: 'GitHub Light', dark: 'GitHub Dark' },
      componentDir: './src/components',
    }),
    svelte(),
  ],
});
