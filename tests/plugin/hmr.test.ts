import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';
import { mogPlugin } from '../../src/plugin/index.js';

it('invalidates a generated framework module when its Mog source changes', async () => {
  const root = await mkdtemp(join(tmpdir(), 'vite-plugin-mog-'));
  const documentPath = join(root, 'document.mg');
  await Promise.all([
    writeFile(join(root, 'entry.js'), "import Document from './document.mg';\nvoid Document;\n"),
    writeFile(documentPath, '# Watched\n'),
  ]);

  const server = await createServer({
    root,
    logLevel: 'silent',
    plugins: [mogPlugin({ mode: 'react' })],
    resolve: {
      alias: {
        'react/jsx-runtime': fileURLToPath(import.meta.resolve('react/jsx-runtime')),
      },
    },
    server: { middlewareMode: true, preTransformRequests: false, watch: null },
  });

  try {
    await server.transformRequest('/entry.js');
    const entryModule = await server.moduleGraph.getModuleByUrl('/entry.js');
    const documentModule = [...(entryModule?.importedModules ?? [])].find(module =>
      module.id?.includes('document.mg.jsx')
    );
    if (!documentModule) throw new Error('generated document module was not loaded');

    await server.transformRequest(documentModule.url);
    expect([...documentModule.importedModules].some(module => module.file === documentPath)).toBe(
      true
    );
    expect(documentModule.transformResult).not.toBeNull();

    server.moduleGraph.onFileChange(documentPath);
    expect(documentModule.transformResult).toBeNull();
  } finally {
    await server.close();
    await rm(root, { recursive: true, force: true });
  }
});
