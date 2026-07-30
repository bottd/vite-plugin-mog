import { resolve } from 'node:path';
import ts from 'typescript';

const cases = ['html', 'react', 'svelte', 'vue', 'metadata'] as const;

it.each(cases)('declares .mg modules for the %s type-reference entry', mode => {
  const root = resolve(import.meta.dirname, '../..');
  const declaration = resolve(root, `src/plugin/types/${mode}.d.ts`);
  const consumer = resolve(root, `tests/types/${mode}.ts`);
  const program = ts.createProgram([declaration, consumer], {
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    target: ts.ScriptTarget.ES2020,
    noEmit: true,
    skipLibCheck: true,
    strict: true,
  });
  const diagnostics = ts.getPreEmitDiagnostics(program);
  const messages = diagnostics.map(diagnostic =>
    ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n')
  );

  expect(messages).toEqual([]);
});
