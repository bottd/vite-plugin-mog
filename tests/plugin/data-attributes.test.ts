import { writeFileSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { mogPlugin, type MogPluginOptions } from '../../src/plugin/index.js';
import { loadCode } from './fixtures.js';

const dir = mkdtempSync(join(tmpdir(), 'mog-data-'));

function document(name: string, source: string): string {
  const path = join(dir, name);
  writeFileSync(path, source);
  return path;
}

const WITH_BLOCKS = document(
  'with-blocks.mg',
  '=card:\n``attr:\nimpact 1\nsize 3\n``\n# A\n\nSome text.\n=\n'
);
const WITHOUT_BLOCKS = document('without-blocks.mg', '=card:\n# A\n\nSome text.\n=\n');

async function render(path: string, dataAttributes?: MogPluginOptions['dataAttributes']) {
  const plugin = mogPlugin({ mode: 'html', include: ['**/*.mg'], dataAttributes });
  const code = await loadCode(plugin, path);
  if (code == null) throw new Error(`no code for ${path}`);
  return code;
}

it('renders no node data attributes by default', async () => {
  // The default is `false`: data in a document is not necessarily data for the
  // DOM, and shipping it to every visitor is the surprising direction.
  expect(await render(WITH_BLOCKS)).not.toContain('data-');
});

it('renders a document with attr blocks exactly as one without them', async () => {
  expect(await render(WITH_BLOCKS, false).then(code => code.replaceAll(WITH_BLOCKS, '<doc>'))).toBe(
    await render(WITHOUT_BLOCKS, false).then(code => code.replaceAll(WITHOUT_BLOCKS, '<doc>'))
  );
});

it.each([
  [true, ['data-impact', 'data-size'], []],
  [['impact'], ['data-impact'], ['data-size']],
] as const)('renders %s as the keys it selects', async (option, present, absent) => {
  const code = await render(WITH_BLOCKS, option as MogPluginOptions['dataAttributes']);

  for (const name of present) expect(code).toContain(name);
  for (const name of absent) expect(code).not.toContain(name);
});

it('leaves root-level attr blocks as metadata whatever the option', async () => {
  const path = document('root.mg', '``attr:\ntitle "Kept"\n``\n\n# A\n');

  for (const option of [false, true, ['title']] as const) {
    expect(await render(path, option)).toContain('"title":"Kept"');
  }
});
