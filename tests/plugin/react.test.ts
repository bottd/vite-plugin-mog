import { join } from 'node:path';
import { mogPlugin } from '../../src/plugin/index.js';
import { fixturesDir, fixtures, loadCode } from './fixtures';

describe('React Generator', () => {
  const plugin = mogPlugin({ mode: 'react', include: ['**/*.mg'] });

  it.each(fixtures)('generates correct output for %s', async fixture => {
    const fixturePath = join(fixturesDir, fixture);
    const code = await loadCode(plugin, fixturePath);
    if (code == null) throw new Error(`no code returned for ${fixture}`);
    expect(code.replaceAll(fixturesDir, '<fixtures>')).toMatchSnapshot();
  });
});
