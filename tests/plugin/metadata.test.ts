import { join } from 'node:path';
import { mogPlugin } from '../../src/plugin/index.js';
import { fixturesDir, loadCode } from './fixtures';

const fixtures = ['basic.mg', 'code-blocks.mg', 'headings.mg', 'images.mg', 'links.mg'];

describe('Metadata Generator', () => {
  describe('mode: metadata', () => {
    it.each(fixtures)('should generate correct metadata module for %s', async fixture => {
      const fixturePath = join(fixturesDir, fixture);
      const plugin = mogPlugin({ mode: 'metadata', include: ['**/*.mg'] });
      expect(await loadCode(plugin, fixturePath)).toMatchSnapshot();
    });

    it('should return undefined for non-mog files', async () => {
      const plugin = mogPlugin({ mode: 'metadata', include: ['**/*.mg'] });
      expect(await loadCode(plugin, 'test.js')).toBeUndefined();
    });
  });

  describe('?metadata query', () => {
    it.each(fixtures)(
      'should generate correct metadata module for %s via ?metadata query',
      async fixture => {
        const fixturePath = join(fixturesDir, fixture);
        const plugin = mogPlugin({ mode: 'html', include: ['**/*.mg'] });
        expect(await loadCode(plugin, `${fixturePath}?metadata`)).toMatchSnapshot();
      }
    );

    it('should return undefined for non-mog files with ?metadata', async () => {
      const plugin = mogPlugin({ mode: 'html', include: ['**/*.mg'] });
      expect(await loadCode(plugin, 'test.js?metadata')).toBeUndefined();
    });

    it('ignores framework embeds when only metadata is requested', async () => {
      const fixturePath = join(fixturesDir, 'embed.mg');
      const metadataPlugin = mogPlugin({ mode: 'metadata', include: ['**/*.mg'] });
      const htmlPlugin = mogPlugin({ mode: 'html', include: ['**/*.mg'] });

      await expect(loadCode(htmlPlugin, `${fixturePath}?metadata`)).resolves.toBe(
        await loadCode(metadataPlugin, fixturePath)
      );
    });
  });

  describe('equivalence', () => {
    it.each(fixtures)(
      'mode: metadata and ?metadata produce identical output for %s',
      async fixture => {
        const fixturePath = join(fixturesDir, fixture);
        const metadataPlugin = mogPlugin({ mode: 'metadata', include: ['**/*.mg'] });
        const htmlPlugin = mogPlugin({ mode: 'html', include: ['**/*.mg'] });

        const [modeCode, queryCode] = await Promise.all([
          loadCode(metadataPlugin, fixturePath),
          loadCode(htmlPlugin, `${fixturePath}?metadata`),
        ]);

        expect(modeCode).toBe(queryCode);
      }
    );
  });

  describe('output format', () => {
    it('should not contain html or CSS imports, but should contain toc', async () => {
      const fixturePath = join(fixturesDir, 'basic.mg');
      const plugin = mogPlugin({ mode: 'metadata', include: ['**/*.mg'] });
      const code = await loadCode(plugin, fixturePath);

      expect(code).not.toContain('export const html');
      expect(code).toContain('export const toc');
      expect(code).not.toContain('virtual:mog-arborium.css');
    });

    it('should contain metadata export and default export', async () => {
      const fixturePath = join(fixturesDir, 'basic.mg');
      const plugin = mogPlugin({ mode: 'metadata', include: ['**/*.mg'] });
      const code = await loadCode(plugin, fixturePath);

      expect(code).toContain('export const metadata');
      expect(code).toContain('export default');
    });
  });

  describe('?metadata on different modes', () => {
    it.each(['html', 'svelte', 'react'] as const)('?metadata works on %s mode', async mode => {
      const fixturePath = join(fixturesDir, 'basic.mg');
      const plugin = mogPlugin({ mode, include: ['**/*.mg'] });
      const code = await loadCode(plugin, `${fixturePath}?metadata`);

      expect(code).toContain('export const metadata');
      expect(code).not.toContain('export const html');
      expect(code).toContain('export const toc');
    });
  });
});
