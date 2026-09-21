import type { ContainerTag, DataAttr, EmbedComponent, Segment } from '@parser';

/**
 * Joins the lines of a generated module, dropping the ones a caller opted out
 * of by passing a falsy value. Nested arrays flatten, so a helper can return
 * several lines or none.
 */
export function lines(...parts: (string | string[] | false | null | undefined)[]): string {
  return parts
    .flat()
    .filter(part => typeof part === 'string')
    .join('\n');
}

export function serializeJs(value: unknown): string {
  const serialized = JSON.stringify(value);
  if (serialized === undefined) return 'undefined';
  return serialized
    .replace(/</g, '\\u003c')
    .replace(/\u2028/g, '\\u2028')
    .replace(/\u2029/g, '\\u2029');
}

/** How one framework spells the parts of a document that differ between them. */
export interface SegmentWriter {
  /**
   * HTML with no element of its own, so it needs whatever wrapper the framework
   * has. A wrapper should be `display: contents`: Svelte has none, and the same
   * document should lay out the same in every mode.
   */
  html(html: string): string;
  /**
   * A lifted element holding nothing but HTML. It can take the string as its
   * own content, which spares a wrapper where one would be invalid or visible
   * to CSS — an `<li>`'s text, say. Left out, the element is written as an
   * opening tag, `html`, and a closing tag. `attrs` comes already spelled.
   */
  leaf?(tag: ContainerTag, attrs: string, html: string): string;
  /** One attribute of a lifted element. */
  attr(name: string, value: string): string;
  /** What the framework calls `class`, when that is not it. */
  className?: string;
}

/** Writes the segments as lines of a template, indented by nesting. */
export function writeSegments(segments: Segment[], writer: SegmentWriter): string[] {
  const out: string[] = [];
  let depth = 0;
  const push = (line: string) => out.push('  '.repeat(depth) + line);

  for (let i = 0; i < segments.length; i++) {
    const segment = segments[i];
    switch (segment.kind) {
      case 'html':
        push(writer.html(segment.html));
        break;
      case 'embed':
        push(`<Embed${segment.index} />`);
        break;
      case 'open': {
        const attrs = [
          ...(segment.classes ? [[writer.className ?? 'class', segment.classes]] : []),
          ...segment.data.map(({ name, value }: DataAttr) => [name, value]),
        ]
          .map(([name, value]) => ` ${writer.attr(name, value)}`)
          .join('');
        const [content, end] = [segments[i + 1], segments[i + 2]];
        if (writer.leaf && content?.kind === 'html' && end?.kind === 'close') {
          push(writer.leaf(segment.tag, attrs, content.html));
          i += 2;
          break;
        }
        push(`<${segment.tag}${attrs}>`);
        depth++;
        break;
      }
      case 'close':
        depth--;
        push(`</${segment.tag}>`);
        break;
    }
  }
  return out;
}

/**
 * The document as one HTML string, when that is all it is — nothing lifted, no
 * component to mount. `undefined` otherwise, so a caller cannot join its way
 * past an element it would have had to build.
 */
export function soleHtml(segments: Segment[]): string | undefined {
  return segments.every(segment => segment.kind === 'html')
    ? segments.map(segment => segment.html).join('')
    : undefined;
}

/** `name={"…"}`, for a template that takes expressions. */
export function attrExpr(name: string, value: string): string {
  return `${name}={${JSON.stringify(value)}}`;
}

export function addEmbedImports(embedComponents: EmbedComponent[], filePath?: string): string[] {
  if (embedComponents.length === 0) return [];
  if (!filePath) throw new Error('A Mog file path is required to import component embeds.');
  return embedComponents.map(
    (_, i) => `import Embed${i} from ${JSON.stringify(`${filePath}?embed=${i}`)};`
  );
}

export function addDocumentCssImport(embedCss: string, filePath?: string): string | null {
  if (!embedCss || !filePath) return null;
  return `import ${JSON.stringify(`virtual:mog-css:${filePath}`)};`;
}
