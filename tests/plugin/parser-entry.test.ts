import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { buildMismatch } from '../../src/parser-entry/check.js';
import { parseMog, parseMogAst, parseMogMetadata } from '../../dist/parser/index.js';
import { fixturesDir } from './fixtures.js';
import {
  ATTRIBUTE_KEYS,
  ATTRIBUTES_KEYS,
  NODE_KEYS,
  SPAN_KEYS,
  VALUE_KEYS,
  describeNode,
} from '../types/parser.js';
import type { MogNode } from '../../dist/parser/index.js';

const root = resolve(import.meta.dirname, '../..');

const documents = [
  ...readdirSync(fixturesDir)
    .filter(name => name.endsWith('.mg'))
    .map(name => join(fixturesDir, name)),
  ...['html', 'react', 'svelte', 'vue'].flatMap(mode => {
    const dir = join(root, 'examples', mode, 'content');
    return readdirSync(dir)
      .filter(name => name.endsWith('.mg'))
      .map(name => join(dir, name));
  }),
];

describe('the native binary check', () => {
  const same = {
    expected: 'abc',
    actual: 'abc',
    packageVersion: () => '0.2.1',
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
      packageVersion: () => '0.3.0',
      binaryVersion: '0.2.1',
    });

    expect(message).toContain('0.3.0');
    expect(message).toContain('0.2.1');
    expect(message).toContain('abc');
    expect(message).toContain('def');
    expect(message).toContain('/dist/napi/index.node');
    expect(message).toContain('link:');
    expect(message).toContain('file:');
  });

  it('does no work on the happy path, which runs on every import', () => {
    const locate = vi.fn(() => 'somewhere');
    const packageVersion = vi.fn(() => '0.2.1');

    expect(buildMismatch({ ...same, locate, packageVersion })).toBeNull();
    expect(locate).not.toHaveBeenCalled();
    expect(packageVersion).not.toHaveBeenCalled();
  });
});

describe('parseMogAst', () => {
  it.each(documents)('parses %s to a stable tree', async path => {
    const source = readFileSync(path, 'utf8');
    const document = await parseMogAst(source);

    expect(document.body).toBeInstanceOf(Array);
    expect(JSON.stringify(document)).toBe(JSON.stringify(await parseMogAst(source)));
  });

  it('reads a number out of a node attr block with no decoding', async () => {
    const document = await parseMogAst(
      '=hero:abrams:\n``attr:\nimpact { all before=0.505 after=0.524 }\n``\n## Abrams\n=\n'
    );

    const impact = document.body[0].attributes?.children?.find(entry => entry.name === 'impact');
    const all =
      impact?.value.kind === 'node'
        ? impact.value.children?.find(entry => entry.name === 'all')
        : undefined;
    const before =
      all?.value.kind === 'node'
        ? all.value.entries?.find(entry => entry.name === 'before')
        : undefined;

    expect(before?.value).toEqual({ kind: 'float', value: 0.505 });
  });

  it('keeps a chain separate from an attr block', async () => {
    const attributes = (await parseMogAst('=hero:abrams:\n``attr:\nk 1\n``\n=\n')).body[0]
      .attributes;

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
    const marker = (await parseMogAst(source)).body[0];
    const block = marker.attributes?.blocks?.[0];
    const fence = marker.fence;
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

describe('the plain projection', () => {
  const documents = ['html', 'react', 'svelte', 'vue'].flatMap(mode => {
    const dir = join(root, 'examples', mode, 'content');
    return readdirSync(dir)
      .filter(name => name.endsWith('.mg'))
      .map(name => join(dir, name));
  });

  it.each(documents)('agrees with parseMog metadata for %s', async path => {
    const source = readFileSync(path, 'utf8');
    // no mode: metadata does not depend on one, and an embed block only parses
    // in the mode it names
    const [ast, rendered, summary] = await Promise.all([
      parseMogAst(source, { plain: true }),
      parseMog(source),
      parseMogMetadata(source),
    ]);

    expect(ast.attributes?.plain ?? {}).toEqual(rendered.metadata);
    expect(summary.metadata).toEqual(rendered.metadata);
    expect(summary.toc).toEqual(rendered.toc);
    expect(Object.keys(summary).sort()).toEqual(['diagnostics', 'metadata', 'toc']);
  });

  it('is absent unless asked for', async () => {
    const source = '``attr:\ntitle "Patch"\n``\n';

    expect((await parseMogAst(source)).attributes?.plain).toBeUndefined();
    expect((await parseMogAst(source, { plain: true })).attributes?.plain).toEqual({
      title: 'Patch',
    });
  });

  it('omits plain when an owner has no attribute children', async () => {
    for (const source of ['=hero:\n# A\n=\n', '=hero:\n``attr:\n``\n=\n']) {
      const { body } = await parseMogAst(source, { plain: true });
      expect(body[0].attributes?.entries).toBeDefined();
      expect(body[0].attributes).not.toHaveProperty('plain');
    }
    const { attributes } = await parseMogAst('``attr:\n``\n', { plain: true });
    expect(attributes?.blocks).toHaveLength(1);
    expect(attributes).not.toHaveProperty('plain');
  });

  it('applies the metadata rule rather than restating the tree', async () => {
    const source =
      '``attr:\ntitle "Patch"\nauthors "John" "Jane"\nversion 1\nauthor name="Drake" {\n  email "a@b.com"\n}\nbig 99999999999999999999\n``\n';
    const { attributes } = await parseMogAst(source, { plain: true });

    expect(attributes?.plain).toEqual({
      title: 'Patch',
      authors: ['John', 'Jane'],
      version: 1,
      author: { name: 'Drake', email: 'a@b.com' },
      big: '99999999999999999999',
    });
  });

  it('keeps the __proto__ guard the metadata rule has', async () => {
    const { attributes } = await parseMogAst('``attr:\ntitle "Kept"\n__proto__ "dropped"\n``\n', {
      plain: true,
    });

    expect(attributes?.plain).toEqual({ title: 'Kept' });
    expect(Object.getPrototypeOf(attributes?.plain)).toBe(Object.prototype);
  });

  it('projects a node attr block, not just the document', async () => {
    const { body } = await parseMogAst('=hero:\n``attr:\nimpact 3\n``\n# A\n=\n', { plain: true });

    expect(body[0].attributes?.plain).toEqual({ impact: 3 });
  });
});

describe('opt-in AST diagnostics', () => {
  it.each([
    '``attr:\nimpact {\n``\n',
    '# Heading\n``attr:\nk 1\n``\n',
    '=hero:\n=ability:\n# Heading\n``attr:\nk 1\n``\n=\n=\n',
    '# Heading\n\nParagraph ``attr: impact {``\n',
    '``attr:\ntitle "T"\n``\n\n``meta:\nlegacy 1\n``\n',
  ])('matches rendering diagnostics for folded and unfolded %s', async source => {
    const { diagnostics } = await parseMog(source);
    expect(diagnostics?.length).toBeGreaterThan(0);
    for (const unfolded of [false, true]) {
      for (const plain of [false, true]) {
        const baseline = await parseMogAst(source, { unfolded, plain });
        const ast = await parseMogAst(source, { unfolded, plain, diagnostics: true });
        expect(ast.diagnostics).toEqual(diagnostics);
        expect(ast).toEqual({ ...baseline, diagnostics });
        expect(baseline).not.toHaveProperty('diagnostics');
      }
    }
  });

  it('includes an empty array only when diagnostics are requested', async () => {
    expect(await parseMogAst('# Clean\n', { diagnostics: true })).toHaveProperty('diagnostics', []);
    expect(await parseMogAst('# Clean\n', { diagnostics: false })).not.toHaveProperty(
      'diagnostics'
    );
  });

  it('keeps plain projection silent even when document diagnostics are requested', async () => {
    const ast = await parseMogAst('``attr:\ntitle "First"\ntitle "Second"\n``\n', {
      plain: true,
      diagnostics: true,
    });
    expect(ast.attributes?.plain).toEqual({ title: 'First' });
    expect(ast.diagnostics).toEqual([]);
  });
});

describe('the hand-written AST declaration', () => {
  it('matches every shape the parser actually emits', async () => {
    const source = readFileSync(join(root, 'tests/fixtures/ast-variants.mg'), 'utf8');
    const document = await parseMogAst(source, { unfolded: true, plain: true });
    const seen = new Set<string>();

    const checkSpan = (span: unknown, where: string) => {
      for (const key of Object.keys(span as object)) {
        expect(SPAN_KEYS, `${where}.${key}`).toContain(key);
      }
    };

    const checkValue = (value: { kind: string }, where: string) => {
      seen.add(`value:${value.kind}`);
      const known = VALUE_KEYS[value.kind as keyof typeof VALUE_KEYS];
      expect(known, `${where} has undeclared value kind ${value.kind}`).toBeDefined();
      for (const key of Object.keys(value)) {
        expect(known, `${where}.${key}`).toContain(key);
      }
      if (value.kind === 'node') {
        const node = value as { entries?: unknown[]; children?: unknown[] };
        for (const entry of [...(node.entries ?? []), ...(node.children ?? [])]) {
          checkAttribute(entry as Record<string, unknown>, `${where}.entry`);
        }
      }
    };

    const checkAttribute = (attribute: Record<string, unknown>, where: string) => {
      for (const key of Object.keys(attribute)) {
        expect(ATTRIBUTE_KEYS, `${where}.${key}`).toContain(key);
      }
      checkValue(attribute.value as { kind: string }, `${where}.value`);
    };

    const checkAttributes = (attributes: Record<string, unknown>, where: string) => {
      for (const key of Object.keys(attributes)) {
        expect(ATTRIBUTES_KEYS, `${where}.${key}`).toContain(key);
      }
      for (const entry of [
        ...((attributes.entries as unknown[]) ?? []),
        ...((attributes.children as unknown[]) ?? []),
      ]) {
        checkAttribute(entry as Record<string, unknown>, where);
      }
      for (const span of (attributes.blocks as unknown[]) ?? []) checkSpan(span, `${where}.blocks`);
    };

    const walk = (nodes: MogNode[]) => {
      for (const node of nodes) {
        seen.add(`kind:${node.kind}`);
        if (node.kind === 'marker') seen.add(`marker:${node.marker}`);
        if (node.kind === 'delimiter') seen.add(`delimiter:${node.delimiter}`);

        // throws on a kind the declaration does not know
        expect(typeof describeNode(node)).toBe('string');

        const known = NODE_KEYS[node.kind];
        for (const key of Object.keys(node)) {
          expect(known, `${node.kind}.${key} is emitted but not declared`).toContain(key);
        }
        if (node.span) checkSpan(node.span, node.kind);
        if (node.fence) checkSpan(node.fence, node.kind);
        if (node.attributes) checkAttributes(node.attributes, node.kind);
        walk(node.children ?? []);
      }
    };

    if (document.attributes) checkAttributes(document.attributes, 'document');
    walk(document.body);

    // the fixture is only a guard while it keeps covering the surface
    expect(seen.size).toBeGreaterThanOrEqual(8 + 5 + 10 + 6);
  });
});
