import { join } from 'node:path';
import { parseMog } from '@parser';
import { mogPlugin } from '../../src/plugin/index.js';
import { fixturesDir, componentsDir } from './fixtures';

async function createPlugin(opts: Parameters<typeof mogPlugin>[0]) {
  const plugin = mogPlugin(opts);
  const hook = plugin.buildStart;
  if (typeof hook === 'function') {
    // Rollup calls buildStart with a plugin context; supply one rather than
    // making the plugin defend against a missing `this`.
    const watched: string[] = [];
    await (hook as (this: { addWatchFile: (id: string) => void }) => Promise<void>).call({
      addWatchFile: (id: string) => watched.push(id),
    });
  }
  return plugin;
}

describe('embed feature', () => {
  describe('with mode', () => {
    it('should parse embed blocks and collect embeds', async () => {
      const content = `
# Test

\`\`embed:svelte:
<script>
  let count = 0;
</script>

<button on:click={() => count++}>
  Count: {count}
</button>
\`\`

# Another heading
`;
      const result = await parseMog(content, 'svelte');

      expect(result.embedComponents).toHaveLength(1);
      expect(result.embedComponents[0].index).toBe(0);
      expect(result.embedComponents[0].mode).toBe('svelte');
      expect(result.embedComponents[0].code).toContain('let count');
      expect(result.htmlParts).toHaveLength(2);
    });

    it('should parse multiple embed blocks', async () => {
      const content = `
# Test

\`\`embed:svelte:
<div>First</div>
\`\`

\`\`embed:svelte:
<div>Second</div>
\`\`
`;
      const result = await parseMog(content, 'svelte');

      expect(result.embedComponents).toHaveLength(2);
      expect(result.embedComponents.map(embed => embed.index)).toEqual([0, 1]);
      expect(result.htmlParts).toHaveLength(3);
    });

    it('should error when language not specified in the block', async () => {
      const content = `
\`\`embed:
let x = 1;
\`\`
`;
      await expect(parseMog(content, 'svelte')).rejects.toThrow(/missing language/i);
    });

    it('should error on invalid language', async () => {
      const content = `
\`\`embed:invalid:
some code
\`\`
`;
      await expect(parseMog(content, 'svelte')).rejects.toThrow(/invalid language/i);
    });

    it('should error when embed language mismatches mode', async () => {
      const content = `
\`\`embed:vue:
<template><div>Hi</div></template>
\`\`
`;
      await expect(parseMog(content, 'svelte')).rejects.toThrow(/cannot be used in svelte mode/i);
    });

    it('should parse react embed blocks in react mode', async () => {
      const content = `
# Test

\`\`embed:react:
<button onClick={() => setCount(c => c + 1)}>Click me</button>
\`\`

# Another heading
`;
      const result = await parseMog(content, 'react');

      expect(result.embedComponents).toHaveLength(1);
      expect(result.embedComponents[0].mode).toBe('react');
      expect(result.embedComponents[0].code).toContain('onClick');
      expect(result.htmlParts).toHaveLength(2);
    });

    it('should error when a react embed is used in svelte mode', async () => {
      const content = `
\`\`embed:react:
<button>Click</button>
\`\`
`;
      await expect(parseMog(content, 'svelte')).rejects.toThrow(/cannot be used in svelte mode/i);
    });
  });

  describe('without mode', () => {
    it('should error on embed blocks when no language specified', async () => {
      const content = `
\`\`embed:
<script>
  let count = 0;
</script>
\`\`
`;
      await expect(parseMog(content, null)).rejects.toThrow(/missing language/i);
    });
  });

  describe('react embed modules', () => {
    const reactFixture = join(fixturesDir, 'embed-react.mg');

    it('should wrap react embed code as JSX component', async () => {
      const plugin = await createPlugin({ mode: 'react', include: ['**/*.mg'] });
      const resolved = (await plugin.resolveId(`${reactFixture}?embed=0`, reactFixture)) as string;
      const result = await plugin.load(resolved);

      expect(result).toContain('export default function MogEmbed() { return <>');
      expect(result).toContain('onClick');
      expect(result).toContain('</>; }');
    });

    it('should inject component imports into react embed modules', async () => {
      const plugin = await createPlugin({
        mode: 'react',
        include: ['**/*.mg'],
        componentDir: componentsDir,
      });
      const resolved = (await plugin.resolveId(`${reactFixture}?embed=0`, reactFixture)) as string;
      const result = await plugin.load(resolved);

      expect(result).toContain('import Badge from "');
      expect(result).toContain('import Counter from "');
      expect(result).toContain('import Typed from "');
      expect(result).toContain('export default function MogEmbed() { return <>');
    });
  });

  describe('componentDir', () => {
    const embedFixture = join(fixturesDir, 'embed.mg');

    it('should inject component imports into embed modules with existing <script>', async () => {
      const plugin = await createPlugin({
        mode: 'svelte',
        include: ['**/*.mg'],
        componentDir: componentsDir,
      });
      const resolved = (await plugin.resolveId(`${embedFixture}?embed=0`, embedFixture)) as string;
      const result = await plugin.load(resolved);

      expect(result).toContain('import Badge from "');
      expect(result).toContain('import Counter from "');
      expect(result).toMatch(/<script>\nimport /);
    });

    it('should inject component imports into embed modules without <script>', async () => {
      const plugin = await createPlugin({
        mode: 'svelte',
        include: ['**/*.mg'],
        componentDir: componentsDir,
      });
      const resolved = (await plugin.resolveId(`${embedFixture}?embed=1`, embedFixture)) as string;
      const result = await plugin.load(resolved);

      expect(result).toContain('import Badge from "');
      expect(result).toContain('import Counter from "');
      expect(result).toMatch(/^<script>\nimport /);
      expect(result).toContain('</script>\n<div>Hello from Svelte!</div>');
    });

    it('should not inject imports when no componentDir is set', async () => {
      const plugin = await createPlugin({ mode: 'svelte', include: ['**/*.mg'] });
      const resolved = (await plugin.resolveId(`${embedFixture}?embed=1`, embedFixture)) as string;
      const result = await plugin.load(resolved);

      expect(result).not.toContain('import Badge');
      expect(result).not.toContain('import Counter');
    });

    it('should not inject imports for non-embed modules', async () => {
      const plugin = await createPlugin({
        mode: 'svelte',
        include: ['**/*.mg'],
        componentDir: componentsDir,
      });
      const result = await plugin.load(embedFixture);

      expect(result).not.toContain(`import Badge from '${componentsDir}`);
    });

    it('preserves explicit component overrides after a directory update', async () => {
      const plugin = await createPlugin({
        mode: 'react',
        include: ['**/*.mg'],
        componentDir: componentsDir,
        components: { Badge: '@/Badge.jsx' },
      });
      const hotUpdate = plugin.hotUpdate as (this: object, ctx: object) => Promise<unknown>;
      await hotUpdate.call(
        { environment: { moduleGraph: {} } },
        { file: join(componentsDir, 'Counter.jsx'), modules: [] }
      );

      const resolved = (await plugin.resolveId(
        `${join(fixturesDir, 'embed-react.mg')}?embed=0`,
        join(fixturesDir, 'embed-react.mg')
      )) as string;
      const result = await plugin.load(resolved);

      expect(result).toContain('import Badge from "@/Badge.jsx";');
    });

    it('finds components in subdirectories', async () => {
      const plugin = await createPlugin({
        mode: 'svelte',
        include: ['**/*.mg'],
        componentDir: componentsDir,
      });
      const resolved = (await plugin.resolveId(`${embedFixture}?embed=1`, embedFixture)) as string;
      const result = await plugin.load(resolved);

      expect(result).toContain('import Deep from "');
      expect(result).toContain('nested/Deep.svelte";');
    });

    it('reports a missing componentDir with the plugin name and path', async () => {
      await expect(
        createPlugin({ mode: 'svelte', include: ['**/*.mg'], componentDir: '/no/such/dir' })
      ).rejects.toThrow(/\[vite-plugin-mog\] Cannot read componentDir "\/no\/such\/dir"/);
    });

    it('rejects component filenames that cannot be imported as bindings', async () => {
      await expect(
        createPlugin({
          mode: 'react',
          include: ['**/*.mg'],
          componentDir: join(fixturesDir, 'invalid-components'),
        })
      ).rejects.toThrow(/not a valid JavaScript identifier/);
    });
  });
});
