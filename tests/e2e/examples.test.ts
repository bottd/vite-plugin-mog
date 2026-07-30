import { join } from 'node:path';
import { build, type Rollup } from 'vite';

// The examples build against dist/, the way a consumer gets the plugin.
const examplesDir = join(import.meta.dirname, '../../examples');

async function bundle(example: string): Promise<string> {
  const result = await build({
    root: join(examplesDir, example),
    logLevel: 'silent',
    // Minified, so the rendered HTML keeps its unescaped double quotes.
    build: { write: false },
  });
  const outputs = (Array.isArray(result) ? result : [result]) as Rollup.RollupOutput[];
  return outputs
    .flatMap(({ output }) => output)
    .map(part => (part.type === 'chunk' ? part.code : String(part.source)))
    .join('\n');
}

it.each([
  { example: 'svelte', heading: 'Mog in Svelte', embed: 'Clicked' },
  { example: 'vue', heading: 'Mog in Vue', embed: 'Clicked' },
  { example: 'react', heading: 'Mog in React', embed: 'Clicked' },
  { example: 'html', heading: 'Mog as HTML', embed: 'Inline markup' },
])('renders the $example example end to end', async ({ example, heading, embed }) => {
  const output = await bundle(example);

  // index.mg: prose and a highlighted code block
  expect(output).toContain(heading);
  expect(output).toContain('class="line"');
  expect(output).toContain('pre.arborium');
  // embeds.mg: the embed and its document CSS
  expect(output).toContain(embed);
  expect(output).toContain('mog-note');
});
