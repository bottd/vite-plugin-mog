import {
  parseMogAst,
  parseMogMetadata,
  type MogMetadataResult,
  type MogAttribute,
  type MogAttributes,
  type MogDocument,
  type MogNode,
  type MogSpan,
  type MogValue,
} from '../../dist/parser/index.js';

// Checked by tests/plugin/types.test.ts, never called: this module is also
// imported at runtime for the key maps below.
export function typeAssertions(node: MogNode, document: MogDocument): void {
  if (node.kind === 'marker') {
    node.marker satisfies 'heading' | 'unordered-list' | 'ordered-list' | 'blockquote' | 'free';
    node.depth satisfies number;
    node.fence?.startLine satisfies number | undefined;
  } else if (node.kind === 'text') {
    node.text satisfies string;
  } else if (node.kind === 'link') {
    node.target satisfies string;
  } else if (node.kind === 'delimiter') {
    node.delimiter satisfies string;
  }

  node.span?.start satisfies number | undefined;
  node.children?.[0]?.kind satisfies string | undefined;

  document.attributes?.blocks?.[0]?.endLine satisfies number | undefined;
  document.attributes?.plain satisfies Record<string, unknown> | undefined;

  const value = document.attributes?.children?.[0]?.value;
  if (value?.kind === 'int') {
    value.value satisfies number | string;
  } else if (value?.kind === 'float') {
    value.value satisfies number;
  } else if (value?.kind === 'node') {
    value.entries?.[0]?.name satisfies string | undefined;
  }

  parseMogAst('', { unfolded: true, plain: true }) satisfies Promise<MogDocument>;
  parseMogAst('') satisfies Promise<MogDocument>;
  parseMogMetadata('') satisfies Promise<MogMetadataResult>;
  // @ts-expect-error Options must remain checked through the public declaration.
  parseMogAst('', { plain: 'true' });
  // @ts-expect-error Unknown options must not silently become accepted.
  parseMogAst('', { unknown: true });
}

// Every key the Rust side can emit, per kind. `satisfies Record<…['kind'], …>`
// makes this exhaustive: a kind added to the declaration stops this compiling,
// and the runtime walk in parser-entry.test.ts fails on a key that is emitted
// but not listed here. `tests/fixtures/ast-variants.mg` is what produces them.
const NODE_COMMON = ['kind', 'attributes', 'children', 'span', 'fence'] as const;

export const NODE_KEYS = {
  marker: [...NODE_COMMON, 'marker', 'depth'],
  delimiter: [...NODE_COMMON, 'delimiter'],
  raw: [...NODE_COMMON, 'text'],
  link: [...NODE_COMMON, 'target'],
  attributes: [...NODE_COMMON],
  paragraph: [...NODE_COMMON],
  table: [...NODE_COMMON],
  text: [...NODE_COMMON, 'text'],
} satisfies Record<MogNode['kind'], readonly string[]>;

export const VALUE_KEYS = {
  null: ['kind'],
  bool: ['kind', 'value'],
  int: ['kind', 'value'],
  float: ['kind', 'value'],
  string: ['kind', 'value'],
  node: ['kind', 'entries', 'children'],
} satisfies Record<MogValue['kind'], readonly string[]>;

export const ATTRIBUTES_KEYS = [
  'entries',
  'children',
  'blocks',
  'plain',
] satisfies (keyof MogAttributes)[];

export const ATTRIBUTE_KEYS = ['name', 'ty', 'value'] satisfies (keyof MogAttribute)[];

export const SPAN_KEYS = ['start', 'end', 'startLine', 'endLine'] satisfies (keyof MogSpan)[];

// An unhandled kind is a type error here, not a surprise at runtime.
export function describeNode(n: MogNode): string {
  switch (n.kind) {
    case 'marker':
      return `${n.marker}:${n.depth}`;
    case 'delimiter':
      return n.delimiter;
    case 'raw':
    case 'text':
      return n.text;
    case 'link':
      return n.target;
    case 'attributes':
    case 'paragraph':
    case 'table':
      return n.kind;
    default: {
      const unhandled: never = n;
      throw new Error(`unhandled node kind: ${JSON.stringify(unhandled)}`);
    }
  }
}
