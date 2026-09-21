import { join } from 'node:path';
import type { ContainerTag, OutputMode, Segment } from '@parser';
import { compile as compileSvelte } from 'svelte/compiler';
import { compileTemplate, parse as parseVue } from 'vue/compiler-sfc';
import { mogPlugin } from '../../src/plugin/index.js';
import { generateOutput, type GeneratorMode } from '../../src/plugin/generators';
import { fixturesDir, loadCode, parseResult } from './fixtures';

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

  describe('a container lifted around an embed', () => {
    const embedComponents = [{ index: 0, mode: 'svelte' as OutputMode, code: '<b>embedded</b>' }];
    // `@parser` is mocked here, so the enum has no runtime value: the names are
    // checked against it as types instead.
    const tag = (name: `${ContainerTag}`) => name as ContainerTag;
    const segments: Segment[] = [
      {
        kind: 'open',
        tag: tag('div'),
        classes: 'card "quoted"',
        data: [{ name: 'data-impact', value: '{"all":{"after":0.524}}' }],
      },
      { kind: 'open', tag: tag('ol'), classes: '', data: [] },
      { kind: 'open', tag: tag('li'), classes: 'task', data: [] },
      { kind: 'html', html: 'one' },
      { kind: 'close', tag: tag('li') },
      { kind: 'open', tag: tag('li'), classes: '', data: [] },
      { kind: 'html', html: 'two' },
      { kind: 'embed', index: 0 },
      { kind: 'close', tag: tag('li') },
      { kind: 'close', tag: tag('ol') },
      { kind: 'close', tag: tag('div') },
    ];
    const generate = (mode: GeneratorMode) =>
      generateOutput(mode, parseResult({ segments, embedComponents }), '', '/doc.mg');

    it.each(['svelte', 'react', 'vue'] as const)('%s nests the embed in its containers', mode => {
      expect(generate(mode)).toMatchSnapshot();
    });

    it('svelte output compiles, with the embed inside the item', () => {
      const code = generate('svelte');
      expect(() => compileSvelte(code, {})).not.toThrow();
      expect(code).toMatch(/<li>\s*\{@html "two"\}\s*<Embed0 \/>\s*<\/li>/);
    });

    it('vue output compiles', () => {
      const { descriptor, errors } = parseVue(generate('vue'));
      expect(errors).toEqual([]);
      const template = compileTemplate({
        source: descriptor.template?.content ?? '',
        filename: 'doc.vue',
        id: 'doc',
      });
      expect(template.errors).toEqual([]);
    });

    it.each([
      ['react', '<li className={"task"} dangerouslySetInnerHTML={{ __html: "one" }} />'],
      ['vue', '<li class="task" v-html="html[0]"></li>'],
    ] as const)('%s gives an html-only item no wrapper', (mode, item) => {
      expect(generate(mode)).toContain(item);
    });
  });

  it('inlines embeds into the html string rather than importing them', async () => {
    const plugin = mogPlugin({ mode: 'html', include: ['**/*.mg'] });
    const code = (await loadCode(plugin, join(fixturesDir, 'embed-html.mg'))) ?? '';

    expect(code).not.toContain('import Embed0');
    expect(code).toContain('<figure class=');
  });
});
