import { fileURLToPath } from 'node:url';
import { join, dirname } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
export const fixturesDir = join(__dirname, '../fixtures');
export const componentsDir = join(fixturesDir, 'components');

// Every fixture without a framework-specific embed block, so each generator can
// render all of them. Embed wiring lives in embed-output.test.ts.
export const fixtures = [
  'basic.mg',
  'blocks.mg',
  'code-blocks.mg',
  'headings.mg',
  'images.mg',
  'links.mg',
  'nested-lists.mg',
  'tasks.mg',
  'embed-css.mg',
];

export async function loadCode(
  plugin: { load?: (id: string) => unknown | Promise<unknown> },
  id: string
): Promise<string | undefined> {
  const result = await plugin.load?.(id);
  if (result == null) return undefined;
  return typeof result === 'string' ? result : (result as { code: string }).code;
}
