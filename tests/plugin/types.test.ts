import { resolve } from 'node:path';
import { typeCheck } from './fixtures.js';

const root = resolve(import.meta.dirname, '../..');

const cases = ['html', 'react', 'svelte', 'vue', 'metadata'] as const;

it.each(cases)('declares .mg modules for the %s type-reference entry', mode => {
  expect(
    typeCheck(
      resolve(root, `src/plugin/types/${mode}.d.ts`),
      resolve(root, `tests/types/${mode}.ts`)
    )
  ).toEqual([]);
});

it('declares a parser tree TypeScript can narrow without casts', () => {
  expect(
    typeCheck(resolve(root, 'dist/parser/index.d.ts'), resolve(root, 'tests/types/parser.ts'), {
      skipLibCheck: false,
    })
  ).toEqual([]);
});
