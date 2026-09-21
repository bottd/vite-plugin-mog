// Hand-written rather than generated: the AST crosses the boundary as JSON, so
// there is no napi object for the type generator to describe, and the generated
// declarations resolve the binding through an internal alias that must not
// appear in a published package.

export type {
  DataAttributes,
  MogParseResult,
  Segment,
  DataAttr,
  ContainerTag,
  EmbedComponent,
  TocEntry,
} from '../napi/index.js';
export {
  parseMog,
  getThemeCss,
  themeNames,
  OutputMode,
  DataAttributesMode,
} from '../napi/index.js';

/** This package's version, and the digest of the tree the binary was built from. */
export declare function version(): string;
export declare function buildId(): string;

/**
 * A node's position in the source string.
 *
 * `start` and `end` are **UTF-8 byte offsets**, half-open. A JavaScript string
 * is indexed in UTF-16 code units, so `content.slice(start, end)` is wrong for
 * any document containing a non-ASCII character. Splice on `startLine` /
 * `endLine`, or slice a `Buffer`.
 *
 * A line terminator belongs to no span, so text inserted directly after a span
 * begins on line `endLine + 1` whatever the file's line endings are.
 */
export interface MogSpan {
  start: number;
  end: number;
  /** 0-based line holding `start`. */
  startLine: number;
  /** 0-based line holding `end - 1`. */
  endLine: number;
}

export type MogMarkerKind =
  | 'heading'
  | 'unordered-list'
  | 'ordered-list'
  | 'blockquote'
  | 'free';

export type MogDelimiterKind =
  | 'strong'
  | 'italic'
  | 'verbatim'
  | 'strikethrough'
  | 'table-header'
  | 'table-row'
  | 'table-cell'
  | 'footnote'
  | 'link'
  | 'link-name';

/**
 * A KDL value. Tagged so it can be narrowed; an integer outside JavaScript's
 * exact range arrives as a string rather than silently losing its low digits,
 * so `kind: 'int'` is `number | string`.
 */
export type MogValue =
  | { kind: 'null' }
  | { kind: 'bool'; value: boolean }
  | { kind: 'int'; value: number | string }
  | { kind: 'float'; value: number }
  | { kind: 'string'; value: string }
  | { kind: 'node'; entries?: MogAttribute[]; children?: MogAttribute[] };

export interface MogAttribute {
  name?: string;
  ty?: string;
  value: MogValue;
}

/**
 * A node's attributes. `entries` are its chain (`=hero:abrams:` gives `hero`
 * and `abrams`); `children` are what its `` ``attr: `` blocks declared. The two
 * are separate namespaces and never merge.
 */
export interface MogAttributes {
  entries?: MogAttribute[];
  children?: MogAttribute[];
  /**
   * Where the `` ``attr: `` blocks behind `children` are, in source order —
   * what a tool replaces to rewrite them. Several blocks merge into one owner,
   * so this is a list.
   *
   * Only blocks written on their own lines appear. One written inside a line
   * has no span of its own, and a span covering the whole line would delete the
   * line. Empty means "nothing to splice", never "no attributes".
   */
  blocks?: MogSpan[];
}

interface MogNodeBase {
  attributes?: MogAttributes;
  children?: MogNode[];
  /**
   * Present on every block-level node. Absent on inline nodes: the inline pass
   * reads text that has already been dedented, unescaped and rejoined, so any
   * offset it reported would be a guess.
   */
  span?: MogSpan;
  /**
   * A marker's opening fence line alone, where that differs from `span` —
   * "insert directly after the fence" is the one position an editor needs that
   * is not a node boundary.
   */
  fence?: MogSpan;
}

export type MogNode =
  | (MogNodeBase & { kind: 'marker'; marker: MogMarkerKind; depth: number })
  | (MogNodeBase & { kind: 'delimiter'; delimiter: MogDelimiterKind })
  | (MogNodeBase & { kind: 'raw'; text: string })
  | (MogNodeBase & { kind: 'link'; target: string })
  | (MogNodeBase & { kind: 'attributes' })
  | (MogNodeBase & { kind: 'paragraph' })
  | (MogNodeBase & { kind: 'table' })
  | (MogNodeBase & { kind: 'text'; text: string });

export interface MogDocument {
  /** The document's root-level `` ``attr: `` blocks, merged in source order. */
  attributes?: MogAttributes;
  body: MogNode[];
}

export interface MogAstOptions {
  /**
   * Keep `` ``attr: `` blocks in the tree as `attributes` nodes instead of
   * folding them into their owner. Default is the folded tree.
   */
  unfolded?: boolean;
}

/**
 * Parses a Mog document to its tree. Renders nothing: no highlighting, no embed
 * extraction, no diagnostics — cheap enough to call for every file in a build.
 */
export declare function parseMogAst(
  content: string,
  options?: MogAstOptions | null
): Promise<MogDocument>;
