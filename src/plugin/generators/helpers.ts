import type { ContainerTag, EmbedComponent, Segment } from '@parser';

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

const ATTR_ESCAPES: Record<string, string> = {
  '&': '&amp;',
  '<': '&lt;',
  '>': '&gt;',
  '"': '&quot;',
  "'": '&#x27;',
};

/** Byte for byte what the renderer's `encode_minimal` writes, so a lifted tag reads like an unlifted one. */
export function escapeAttr(value: string): string {
  return value.replace(/[&<>"']/g, char => ATTR_ESCAPES[char]);
}

/**
 * How one generator spells each step of the document. `slot` numbers the HTML
 * strings in order, for a generator that ships them as an array.
 */
export interface SegmentWriter {
  /**
   * HTML with no element of its own, so it needs whatever wrapper the framework
   * has. A wrapper should be `display: contents`: Svelte and
   * html mode have none, and the same document should lay out the same in all.
   */
  html(html: string, slot: number): string;
  embed(index: number): string;
  open(tag: ContainerTag, classes: string): string;
  close(tag: ContainerTag): string;
  /**
   * A lifted element holding nothing but HTML. It can take the string as its
   * own content, which spares a wrapper where one would be invalid or visible
   * to CSS — an `<li>`'s text, say. Left out, the element is written as
   * `open`, `html`, `close`.
   */
  leaf?(tag: ContainerTag, classes: string, html: string, slot: number): string;
}

/** Writes the segments as lines of a template, indented by nesting. */
export function writeSegments(segments: Segment[], writer: SegmentWriter): string[] {
  const out: string[] = [];
  let depth = 0;
  let slot = 0;
  const push = (line: string) => out.push('  '.repeat(depth) + line);

  for (let i = 0; i < segments.length; i++) {
    const segment = segments[i];
    switch (segment.kind) {
      case 'html':
        push(writer.html(segment.html, slot++));
        break;
      case 'embed':
        push(writer.embed(segment.index));
        break;
      case 'open': {
        const [content, end] = [segments[i + 1], segments[i + 2]];
        if (writer.leaf && content?.kind === 'html' && end?.kind === 'close') {
          push(writer.leaf(segment.tag, segment.classes, content.html, slot++));
          i += 2;
          break;
        }
        push(writer.open(segment.tag, segment.classes));
        depth++;
        break;
      }
      case 'close':
        depth--;
        push(writer.close(segment.tag));
        break;
    }
  }
  return out;
}

/** The HTML strings `writeSegments` numbers, in slot order. */
export function htmlSlots(segments: Segment[]): string[] {
  return segments.flatMap(segment => (segment.kind === 'html' ? [segment.html] : []));
}

/** The document as one HTML string, with each embed replaced by `embed(index)`. */
export function joinSegments(segments: Segment[], embed: (index: number) => string): string {
  return segments
    .map(segment => {
      switch (segment.kind) {
        case 'html':
          return segment.html;
        case 'embed':
          return embed(segment.index);
        case 'open':
          return `<${segment.tag}${classAttr(segment.classes)}>`;
        case 'close':
          return `</${segment.tag}>`;
      }
    })
    .join('');
}

export function classAttr(classes: string): string {
  return classes ? ` class="${escapeAttr(classes)}"` : '';
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
