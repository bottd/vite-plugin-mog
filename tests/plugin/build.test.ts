import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build, type Plugin, type Rollup } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import vue from '@vitejs/plugin-vue';
import { mogPlugin, type MogPluginOptions } from '../../src/plugin/index.js';

const reactAlias = {
  'react/jsx-runtime': fileURLToPath(import.meta.resolve('react/jsx-runtime')),
  'react/jsx-dev-runtime': fileURLToPath(import.meta.resolve('react/jsx-dev-runtime')),
};

// Inside the repo, not tmpdir: a scaffolded project has no node_modules of its
// own, so it has to be somewhere Node can walk up to this repo's.
const scratchRoot = join(import.meta.dirname, '../.tmp');

async function scaffold(files: Record<string, string>): Promise<string> {
  await mkdir(scratchRoot, { recursive: true });
  const root = await mkdtemp(join(scratchRoot, 'build-'));
  for (const [name, content] of Object.entries(files)) {
    const full = join(root, name);
    await mkdir(dirname(full), { recursive: true });
    await writeFile(full, content);
  }
  return root;
}

async function bundle(
  root: string,
  entry: string,
  options: MogPluginOptions,
  framework: Plugin[] = []
) {
  const result = await build({
    root,
    logLevel: 'silent',
    plugins: [mogPlugin(options), ...framework],
    resolve: { alias: reactAlias },
    build: {
      write: false,
      minify: false,
      lib: { entry: join(root, entry), formats: ['es'], fileName: 'out' },
    },
  });
  const output = (result as Rollup.RollupOutput[])[0].output;
  return {
    code: output
      .filter(chunk => chunk.type === 'chunk')
      .map(chunk => chunk.code)
      .join('\n'),
    css: output
      .filter(asset => asset.type === 'asset' && asset.fileName.endsWith('.css'))
      .map(asset => String((asset as Rollup.OutputAsset).source))
      .join('\n'),
  };
}

it('builds a Mog file used directly as a build entry', async () => {
  // With no importer there is nothing to resolve against, so the entry used to
  // skip the `.mg.jsx` rewrite and ship uncompiled JSX.
  const root = await scaffold({ 'doc.mg': '# Entry\n' });
  try {
    const { code } = await bundle(root, 'doc.mg', { mode: 'react' });
    expect(code).not.toContain('dangerouslySetInnerHTML={');
    expect(code).toContain('Entry');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

it('bundles embedded components and document CSS', async () => {
  const root = await scaffold({
    'components/Counter.jsx': 'export default function Counter() { return <b>counted</b>; }\n',
    'doc.mg': [
      '# Title',
      '',
      'before',
      '',
      '``embed:css:',
      'h1 { color: rebeccapurple }',
      '``',
      '',
      '``embed:react:',
      '<Counter />',
      '``',
      '',
      'after',
      '',
    ].join('\n'),
    'entry.jsx': "import Doc from './doc.mg';\nexport default Doc;\n",
  });
  try {
    const { code, css } = await bundle(root, 'entry.jsx', {
      mode: 'react',
      componentDir: './components',
    });

    expect(code).toContain('counted');
    expect(code).toContain('Title');
    expect(code).toContain('after');
    expect(css).toContain('rebeccapurple');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

// The svelte and vue modes hand a generated id ending in .svelte or .vue to
// someone else's plugin. Nothing tested that contract, and vue was broken: the
// id used to carry a `\0` prefix, which Vite's createFilter rejects outright.
it('compiles a Mog document through vite-plugin-svelte', async () => {
  const root = await scaffold({
    'components/Counter.svelte': '<button>counted</button>\n',
    'doc.mg': [
      '# Svelte Title',
      '',
      'before the embed',
      '',
      '``embed:css:',
      'h1 { color: rebeccapurple }',
      '``',
      '',
      '``embed:svelte:',
      '<Counter />',
      '``',
      '',
      'after the embed',
      '',
    ].join('\n'),
    'entry.js': "import Doc, { metadata } from './doc.mg';\nexport { Doc, metadata };\n",
  });
  try {
    const { code, css } = await bundle(
      root,
      'entry.js',
      { mode: 'svelte', componentDir: './components' },
      [svelte({ compilerOptions: { hmr: false } }) as unknown as Plugin]
    );

    // Compiled, not passed through: the SFC source is gone from the bundle.
    expect(code).not.toContain('<script lang="ts" module>');
    expect(code).toContain('Svelte Title');
    expect(code).toContain('counted');
    expect(css).toContain('rebeccapurple');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

it('compiles a Mog document through @vitejs/plugin-vue', async () => {
  const root = await scaffold({
    'components/Counter.vue': '<template><button>counted</button></template>\n',
    'doc.mg': [
      '# Vue Title',
      '',
      'before the embed',
      '',
      '``embed:css:',
      'h1 { color: rebeccapurple }',
      '``',
      '',
      '``embed:vue:',
      '<Counter />',
      '``',
      '',
      'after the embed',
      '',
    ].join('\n'),
    'entry.js': "import Doc, { metadata } from './doc.mg';\nexport { Doc, metadata };\n",
  });
  try {
    const { code, css } = await bundle(
      root,
      'entry.js',
      { mode: 'vue', componentDir: './components' },
      [vue() as Plugin]
    );

    expect(code).not.toContain('<template>');
    expect(code).toContain('Vue Title');
    expect(code).toContain('counted');
    expect(css).toContain('rebeccapurple');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
