import { join } from 'node:path';
import { mogPlugin } from '../../src/plugin/index.js';
import { generateVue } from '../../src/plugin/generators/vue.js';
import { fixturesDir, fixtures, loadCode, parseResult } from './fixtures';

describe('Vue Generator', () => {
  const plugin = mogPlugin({ mode: 'vue', include: ['**/*.mg'] });

  it.each(fixtures)('generates correct output for %s', async fixture => {
    const fixturePath = join(fixturesDir, fixture);
    const code = await loadCode(plugin, fixturePath);
    if (code == null) throw new Error(`no code returned for ${fixture}`);
    expect(code.replaceAll(fixturesDir, '<fixtures>')).toMatchSnapshot();
  });

  it('escapes script data and imports document CSS as a virtual module', () => {
    const result = parseResult({
      metadata: { title: '</script><div>broken</div>' },
      segments: [{ kind: 'html', html: '<p>Safe</p>' }],
      embedCss: 'p::before { content: "</style>"; }',
    });
    const code = generateVue(result, '', '/tmp/document.mg');

    expect(code).not.toContain('</script><div>broken</div>');
    expect(code).toContain('\\u003c/script>');
    expect(code).toContain('import "virtual:mog-css:/tmp/document.mg";');
    expect(code).not.toContain('<style>');
  });
});
