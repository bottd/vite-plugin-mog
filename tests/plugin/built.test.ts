import { existsSync } from 'node:fs';
import { resolve } from 'node:path';

// Every other test imports plugin source with `@parser` mocked, so nothing
// exercises what is actually published: the bundle resolves `@parser` to
// `../parser/index.js` at build time, and an export the entry does not
// re-export fails only there.
const root = resolve(import.meta.dirname, '../..');
const bundle = resolve(root, 'dist/plugin/index.js');

describe.skipIf(!existsSync(bundle))('the built bundle', () => {
  it('loads and resolves every import it makes', async () => {
    vi.doUnmock('@parser');
    const module = await import(bundle);

    expect(typeof module.mogPlugin).toBe('function');
    expect(module.mogPlugin({ mode: 'html' }).name).toBe('vite-plugin-mog');
  });

  it('exposes the parser entry it was built against', async () => {
    const parser = await import(resolve(root, 'dist/parser/index.js'));

    for (const name of ['parseMog', 'parseMogAst', 'OutputMode', 'DataAttributesMode']) {
      expect(parser[name], name).toBeDefined();
    }
  });
});
