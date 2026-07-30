import vue from '@vitejs/plugin-vue';
import { mogPlugin } from 'vite-plugin-mog';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [
    mogPlugin({
      mode: 'vue',
      theme: { light: 'GitHub Light', dark: 'GitHub Dark' },
      componentDir: './src/components',
    }),
    vue(),
  ],
});
