import type { MogParseResult } from '@parser';
import {
  addDocumentCssImport,
  addEmbedImports,
  attrExpr,
  lines,
  serializeJs,
  writeSegments,
} from './helpers.js';

export function generateSvelte(
  { segments, metadata, toc, embedComponents = [], embedCss = '' }: MogParseResult,
  css: string,
  filePath?: string
): string {
  const hasImports = !!(css || embedCss || embedComponents.length);
  // Values go through an expression: a `{` in a quoted attribute would open one.
  const body = writeSegments(segments, {
    html: html => `{@html ${JSON.stringify(html)}}`,
    attr: attrExpr,
  });

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
