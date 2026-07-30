import type { EmbedComponent } from '../types/parser.js';

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
