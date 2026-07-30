import { mogPlugin } from 'vite-plugin-mog';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [
    mogPlugin({
      mode: 'html',
      theme: { light: 'GitHub Light', dark: 'GitHub Dark' },
    }),
  ],
  build: {
    rollupOptions: {
      input: { main: 'index.html', embeds: 'embeds.html' },
    },
  },
});
