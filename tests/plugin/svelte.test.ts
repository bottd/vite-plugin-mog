import { join } from 'node:path';
import { compile } from 'svelte/compiler';
import { mogPlugin } from '../../src/plugin/index.js';
import { generateSvelte } from '../../src/plugin/generators/svelte.js';
import { fixturesDir, fixtures, loadCode, parseResult } from './fixtures';

describe('Svelte Generator', () => {
  const plugin = mogPlugin({ mode: 'svelte', include: ['**/*.mg'] });

  it.each(fixtures)('generates correct output for %s', async fixture => {
    const fixturePath = join(fixturesDir, fixture);
    const code = await loadCode(plugin, fixturePath);
    if (code == null) throw new Error(`no code returned for ${fixture}`);
    expect(code.replaceAll(fixturesDir, '<fixtures>')).toMatchSnapshot();
  });

  it('escapes metadata that could terminate a script block', () => {
    const result = parseResult({
      metadata: { title: '</script><div>broken</div>' },
      segments: [{ kind: 'html', html: '<p>Safe</p>' }],
    });
    const code = generateSvelte(result, '', '/tmp/document.mg');

    expect(code).not.toContain('</script><div>broken</div>');
    expect(code).toContain('\\u003c/script>');
    expect(() => compile(code, {})).not.toThrow();
  });
});
