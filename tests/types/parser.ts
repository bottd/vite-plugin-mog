import { parseMogAst, type MogDocument, type MogNode } from '../../dist/parser/index.js';

declare const node: MogNode;

// Narrowing on `kind` alone, with no casts.
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

declare const document: MogDocument;
document.attributes?.blocks?.[0]?.endLine satisfies number | undefined;

const value = document.attributes?.children?.[0]?.value;
if (value?.kind === 'int') {
  // An integer outside JavaScript's exact range arrives as a string.
  value.value satisfies number | string;
} else if (value?.kind === 'float') {
  value.value satisfies number;
} else if (value?.kind === 'node') {
  value.entries?.[0]?.name satisfies string | undefined;
}

parseMogAst('', { unfolded: true }) satisfies Promise<MogDocument>;
parseMogAst('') satisfies Promise<MogDocument>;
