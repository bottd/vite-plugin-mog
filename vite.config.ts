import { defineConfig } from 'vite';
import dts from 'vite-plugin-dts';

export default defineConfig({
  plugins: [
    dts({
      include: ['src/**/*'],
      exclude: ['src/parser/**/*'],
      outDir: 'dist',
      copyDtsFiles: true,
    }),
  ],
  build: {
    lib: {
      entry: 'src/plugin/index.ts',
      name: 'VitePluginMog',
      fileName: 'plugin/index',
      formats: ['es'],
    },
    rollupOptions: {
      external: ['vite', 'node:fs/promises', 'node:path', '@parser'],
      output: {
        paths: {
          // Resolved relative to dist/plugin/index.js, where the bundle lands.
          '@parser': '../napi/index.js',
        },
      },
    },
    copyPublicDir: false,
    outDir: 'dist',
    // needed to preserve build:napi output
    emptyOutDir: false,
  },
});
