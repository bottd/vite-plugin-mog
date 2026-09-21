import { resolve } from 'node:path';

// Every other test imports plugin source with `@parser` mocked, so nothing
// exercises what is actually published. The bundle resolves `@parser` to
// `../parser/index.js` at build time, so importing it here is what proves the
// entry re-exports everything the plugin reaches for — an ESM link error is the
// only way that failure surfaces.
const bundle = resolve(import.meta.dirname, '../../dist/plugin/index.js');

it('loads the built bundle and resolves every import it makes', async () => {
  vi.doUnmock('@parser');
  const module = await import(bundle);

  expect(module.mogPlugin({ mode: 'html' }).name).toBe('vite-plugin-mog');
});
