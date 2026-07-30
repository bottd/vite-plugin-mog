import react from '@vitejs/plugin-react';
import { mogPlugin } from 'vite-plugin-mog';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [
    mogPlugin({
      mode: 'react',
      theme: { light: 'GitHub Light', dark: 'GitHub Dark' },
      componentDir: './src/components',
    }),
    react(),
  ],
});
