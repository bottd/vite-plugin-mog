import type { MogParseResult } from '@parser';
import {
  addDocumentCssImport,
  addEmbedImports,
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
  // Classes go through an expression: a `{` in a quoted attribute would open one.
  const body = writeSegments(segments, {
    html: html => `{@html ${JSON.stringify(html)}}`,
    embed: i => `<Embed${i} />`,
    open: (tag, classes) => (classes ? `<${tag} class={${JSON.stringify(classes)}}>` : `<${tag}>`),
    close: tag => `</${tag}>`,
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
