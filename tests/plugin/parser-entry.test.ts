import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import ts from 'typescript';
import { buildMismatch } from '../../src/parser-entry/check.js';
import { parseMogAst } from '../../dist/parser/index.js';
import { fixturesDir } from './fixtures.js';

const root = resolve(import.meta.dirname, '../..');

function documents(): [string, string][] {
  const files = readdirSync(fixturesDir)
    .filter(name => name.endsWith('.mg'))
    .map(name => join(fixturesDir, name));

  for (const mode of ['html', 'react', 'svelte', 'vue']) {
    const dir = join(root, 'examples', mode, 'content');
    files.push(...readdirSync(dir).map(name => join(dir, name)));
  }
  return files.map(path => [path, readFileSync(path, 'utf8')]);
}

describe('the native binary check', () => {
  const same = {
    expected: 'abc',
    actual: 'abc',
    packageVersion: '0.2.1',
    binaryVersion: '0.2.1',
    locate: () => '/dist/napi/index.node',
  };

  it('passes when both halves came from the same tree', () => {
    expect(buildMismatch(same)).toBeNull();
  });

  it('names both versions, both builds and where the binary came from', () => {
    const message = buildMismatch({
      ...same,
      actual: 'def',
      packageVersion: '0.3.0',
      binaryVersion: '0.2.1',
    });

    expect(message).toContain('0.3.0');
    expect(message).toContain('0.2.1');
    expect(message).toContain('abc');
    expect(message).toContain('def');
    expect(message).toContain('/dist/napi/index.node');
  });

  it('explains the file: fallback that makes versions agree while builds do not', () => {
    const message = buildMismatch({ ...same, actual: 'def' });

    expect(message).toContain('link:');
    expect(message).toContain('file:');
  });

  it('only looks for the binary when there is an error to write', () => {
    const locate = vi.fn(() => 'somewhere');

    expect(buildMismatch({ ...same, locate })).toBeNull();
    expect(locate).not.toHaveBeenCalled();
  });
});

describe('parseMogAst', () => {
  it.each(documents())('parses %s to a stable tree', async (_path, source) => {
    const document = await parseMogAst(source);

    expect(document.body).toBeInstanceOf(Array);
    expect(JSON.stringify(document)).toBe(JSON.stringify(await parseMogAst(source)));
  });

  it('reads a number out of a node attr block with no decoding', async () => {
    const document = await parseMogAst(
      '=hero:abrams:\n``attr:\nimpact { all before=0.505 after=0.524 }\n``\n## Abrams\n=\n'
    );

    const marker = document.body[0];
    const impact = marker.attributes?.children?.find(entry => entry.name === 'impact');
    expect(impact?.value.kind).toBe('node');

    const all =
      impact?.value.kind === 'node'
        ? impact.value.children?.find(entry => entry.name === 'all')
        : undefined;
    const before =
      all?.value.kind === 'node'
        ? all.value.entries?.find(entry => entry.name === 'before')
        : undefined;

    expect(before?.value.kind).toBe('float');
    expect(before?.value.kind === 'float' && before.value.value).toBe(0.505);
  });

  it('keeps a chain separate from an attr block', async () => {
    const document = await parseMogAst('=hero:abrams:\n``attr:\nk 1\n``\n=\n');
    const attributes = document.body[0].attributes;

    expect(
      attributes?.entries?.map(entry => entry.value.kind === 'string' && entry.value.value)
    ).toEqual(['hero', 'abrams']);
    expect(attributes?.children?.map(entry => entry.name)).toEqual(['k']);
  });

  it('keeps attr blocks in the tree when unfolded', async () => {
    const folded = await parseMogAst('=hero:\n``attr:\nk 1\n``\n=\n');
    const unfolded = await parseMogAst('=hero:\n``attr:\nk 1\n``\n=\n', { unfolded: true });

    expect(folded.body[0].children ?? []).toEqual([]);
    expect(unfolded.body[0].children?.map(node => node.kind)).toEqual(['attributes']);
  });

  it('carries spans a tool can splice on', async () => {
    const source = '=hero:\n``attr:\nk 1\n``\n## A\n=\n';
    const document = await parseMogAst(source);
    const block = document.body[0].attributes?.blocks?.[0];

    const fence = document.body[0].fence;
    if (!block || !fence) throw new Error('expected a block span and a fence');

    expect(source.slice(block.start, block.end)).toBe('``attr:\nk 1\n``');
    expect(source.slice(fence.start, fence.end)).toBe('=hero:');
  });

  it('emits an out-of-range integer as a string rather than rounding it', async () => {
    const document = await parseMogAst('``attr:\nbig 99999999999999999999\n``\n');
    const big = document.attributes?.children?.[0];
    const argument = big?.value.kind === 'node' ? big.value.entries?.[0] : undefined;

    expect(argument?.value).toEqual({ kind: 'int', value: '99999999999999999999' });
  });
});

describe('the vite-plugin-mog/parser entry', () => {
  it('imports nothing but node builtins and its own files', () => {
    const imports = ['index.js', 'check.js'].flatMap(name => {
      const source = readFileSync(join(root, 'dist', 'parser', name), 'utf8');
      return [...source.matchAll(/from\s+'([^']+)'/g)].map(match => match[1]);
    });

    expect(imports.length).toBeGreaterThan(0);
    for (const specifier of imports) {
      expect(specifier.startsWith('node:') || specifier.startsWith('.')).toBe(true);
    }
  });

  it('declares a tree TypeScript can narrow without casts', () => {
    const program = ts.createProgram(
      [resolve(root, 'dist/parser/index.d.ts'), resolve(root, 'tests/types/parser.ts')],
      {
        module: ts.ModuleKind.ESNext,
        moduleResolution: ts.ModuleResolutionKind.Bundler,
        target: ts.ScriptTarget.ES2020,
        noEmit: true,
        skipLibCheck: true,
        strict: true,
      }
    );

    const messages = ts
      .getPreEmitDiagnostics(program)
      .map(diagnostic => ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n'));

    expect(messages).toEqual([]);
  });
});
