import { mkdtemp, readdir, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { build } from 'vite';

// The examples build against dist/, the way a consumer gets the plugin.
const examplesDir = join(import.meta.dirname, '../../examples');

async function buildExample(example: string): Promise<{ names: string[]; text: string }> {
  const outDir = await mkdtemp(join(tmpdir(), `mog-e2e-${example}-`));
  try {
    await build({
      root: join(examplesDir, example),
      logLevel: 'silent',
      build: { outDir, emptyOutDir: true },
    });
    const entries = await readdir(outDir, { recursive: true, withFileTypes: true });
    const files = entries.filter(entry => entry.isFile());
    const contents = await Promise.all(
      files.map(file => readFile(join(file.parentPath, file.name), 'utf8'))
    );
    return { names: files.map(file => file.name), text: contents.join('\n') };
  } finally {
    await rm(outDir, { recursive: true, force: true });
  }
}

it.each([
  { example: 'svelte', heading: 'Mog in Svelte', embed: 'Clicked', pages: ['index.html'] },
  { example: 'vue', heading: 'Mog in Vue', embed: 'Clicked', pages: ['index.html'] },
  { example: 'react', heading: 'Mog in React', embed: 'Clicked', pages: ['index.html'] },
  {
    example: 'html',
    heading: 'Mog as HTML',
    embed: 'Inline markup',
    // Two entries, per examples/html/vite.config.ts.
    pages: ['index.html', 'embeds.html'],
  },
])('builds the $example example end to end', async ({ example, heading, embed, pages }) => {
  const { names, text } = await buildExample(example);

  expect(names).toEqual(expect.arrayContaining(pages));
  expect(names.some(name => name.endsWith('.js'))).toBe(true);

  // index.mg: prose and a highlighted code block
  expect(text).toContain(heading);
  expect(text).toContain('class="line"');
  expect(text).toContain('pre.arborium');
  // embeds.mg: the embed and its document CSS
  expect(text).toContain(embed);
  expect(text).toContain('mog-note');
});
