import { join } from 'node:path';
import { compile } from 'svelte/compiler';
import type { MogParseResult } from '@parser';
import { mogPlugin } from '../../src/plugin/index.js';
import { generateSvelte } from '../../src/plugin/generators/svelte.js';
import { fixturesDir, fixtures, loadCode } from './fixtures';

describe('Svelte Generator', () => {
  const plugin = mogPlugin({ mode: 'svelte', include: ['**/*.mg'] });

  it.each(fixtures)('generates correct output for %s', async fixture => {
    const fixturePath = join(fixturesDir, fixture);
    const code = await loadCode(plugin, fixturePath);
    if (code == null) throw new Error(`no code returned for ${fixture}`);
    expect(code.replaceAll(fixturesDir, '<fixtures>')).toMatchSnapshot();
  });

  it('escapes metadata that could terminate a script block', () => {
    const result: MogParseResult = {
      metadata: { title: '</script><div>broken</div>' },
      htmlParts: ['<p>Safe</p>'],
      toc: [],
      embedComponents: [],
      embedCss: '',
    };
    const code = generateSvelte(result, '', '/tmp/document.mg');

    expect(code).not.toContain('</script><div>broken</div>');
    expect(code).toContain('\\u003c/script>');
    expect(() => compile(code, {})).not.toThrow();
  });
});
