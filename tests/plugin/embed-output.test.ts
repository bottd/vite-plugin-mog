import { join } from 'node:path';
import { mogPlugin } from '../../src/plugin/index.js';
import type { GeneratorMode } from '../../src/plugin/generators';
import { fixturesDir, loadCode } from './fixtures';

// The document module that imports `<Embed0 />` is the most intricate code any
// generator emits, and no snapshot covered it. One fixture per mode, because an
// embed block only parses in the mode it names.
const embedFixtures: [Exclude<GeneratorMode, 'metadata'>, string][] = [
  ['html', 'embed-html.mg'],
  ['svelte', 'embed.mg'],
  ['react', 'embed-react.mg'],
  ['vue', 'embed-vue.mg'],
];

describe('document modules with component embeds', () => {
  it.each(embedFixtures)('wires %s embeds into the document module', async (mode, fixture) => {
    const fixturePath = join(fixturesDir, fixture);
    const plugin = mogPlugin({ mode, include: ['**/*.mg'] });
    const code = await loadCode(plugin, fixturePath);
    if (code == null) throw new Error(`no code returned for ${fixture}`);
    expect(code.replaceAll(fixturesDir, '<fixtures>')).toMatchSnapshot();
  });

  it.each(embedFixtures.filter(([mode]) => mode !== 'html'))(
    '%s imports one embed module per embed block',
    async (mode, fixture) => {
      const fixturePath = join(fixturesDir, fixture);
      const plugin = mogPlugin({ mode, include: ['**/*.mg'] });
      const code = (await loadCode(plugin, fixturePath)) ?? '';

      const imports = [...code.matchAll(/import Embed(\d+) from "([^"]+)"/g)];
      const rendered = [...code.matchAll(/<Embed(\d+)\s*\/>/g)];
      expect(imports.length).toBeGreaterThan(0);
      expect(imports.map(match => match[1])).toEqual(rendered.map(match => match[1]));
      for (const [, index, specifier] of imports) {
        expect(specifier).toBe(`${fixturePath}?embed=${index}`);
      }
    }
  );

  it('inlines embeds into the html string rather than importing them', async () => {
    const plugin = mogPlugin({ mode: 'html', include: ['**/*.mg'] });
    const code = (await loadCode(plugin, join(fixturesDir, 'embed-html.mg'))) ?? '';

    expect(code).not.toContain('import Embed0');
    expect(code).toContain('<figure class=');
  });
});
