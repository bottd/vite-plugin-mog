import { readdirSync } from 'node:fs';
import { join } from 'node:path';

export const fixturesDir = join(import.meta.dirname, '../fixtures');
export const componentsDir = join(fixturesDir, 'components');

// Every fixture each generator can render. The framework-specific embed
// fixtures need wiring that lives in embed-output.test.ts, and diagnostics.mg
// exists to exercise warnings rather than clean output.
const notRenderable = new Set([
  'diagnostics.mg',
  'embed.mg',
  'embed-html.mg',
  'embed-react.mg',
  'embed-vue.mg',
]);

export const fixtures = readdirSync(fixturesDir).filter(
  name => name.endsWith('.mg') && !notRenderable.has(name)
);

export async function loadCode(
  plugin: { load?: (id: string) => unknown | Promise<unknown> },
  id: string
): Promise<string | undefined> {
  const result = await plugin.load?.(id);
  if (result == null) return undefined;
  return typeof result === 'string' ? result : (result as { code: string }).code;
}
