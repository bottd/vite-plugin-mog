import type { MogParseResult } from '../types/parser.js';
import { addDocumentCssImport, addEmbedImports, lines, serializeJs } from './helpers.js';

export function generateSvelte(
  { htmlParts, metadata, toc, embedComponents = [], embedCss = '' }: MogParseResult,
  css: string,
  filePath?: string
): string {
  const hasImports = !!(css || embedCss || embedComponents.length);
  const body = htmlParts
    .flatMap((part, i) => [
      `{@html ${JSON.stringify(part)}}`,
      ...(i < embedComponents.length ? [`<Embed${i} />`] : []),
    ])
    .join('\n');

  return lines(
    '<script lang="ts" module>',
    `export const metadata = ${serializeJs(metadata ?? {})};`,
    `export const toc = ${serializeJs(toc ?? [])};`,
    '</script>',
    hasImports && '<script lang="ts">',
    css ? 'import "virtual:mog-arborium.css";' : null,
    addDocumentCssImport(embedCss, filePath),
    addEmbedImports(embedComponents, filePath),
    hasImports && '</script>',
    body
  );
}
