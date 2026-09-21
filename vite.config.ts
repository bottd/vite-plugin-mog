import { defineConfig } from 'vite';
import dts from 'vite-plugin-dts';

// scripts/build-js.mjs builds into a staging directory and renames the result
// over dist, so the package never loses its entry point mid-build. Both the
// bundle and the declarations have to follow it there.
const outDir = process.env.MOG_OUT_DIR ?? 'dist';

export default defineConfig({
  plugins: [
    dts({
      include: ['src/**/*'],
      // src/parser is the Rust crate; src/parser-entry ships as source.
      exclude: ['src/parser/**/*', 'src/parser-entry/**/*'],
      outDir,
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
      external: [
        'vite',
        'node:fs/promises',
        'node:path',
        '@parser',
        'svelte/compiler',
        'vue/compiler-sfc',
      ],
      output: {
        paths: {
          // Resolved relative to dist/plugin/index.js, where the bundle lands.
          // The plugin goes through the public parser entry rather than the
          // generated binding, so it gets the same build check consumers do.
          '@parser': '../parser/index.js',
        },
      },
    },
    copyPublicDir: false,
    outDir,
    // needed to preserve build:napi output
    emptyOutDir: false,
  },
});
