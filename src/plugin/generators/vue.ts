import type { MogParseResult } from '../types/parser.js';
import { addDocumentCssImport, addEmbedImports, lines, serializeJs } from './helpers.js';

export function generateVue(
  { htmlParts, metadata, toc, embedComponents = [], embedCss = '' }: MogParseResult,
  css: string,
  filePath?: string
): string {
  const templateContent =
    embedComponents.length === 0
      ? '<div v-html="htmlContent"></div>'
      : `<div>\n${htmlParts
          .flatMap((part, i) => [
            `  <div v-html="htmlParts[${i}]"></div>`,
            ...(i < embedComponents.length ? [`  <Embed${i} />`] : []),
          ])
          .join('\n')}\n</div>`;

  return lines(
    '<script lang="ts">',
    `export const metadata = ${serializeJs(metadata ?? {})};`,
    `export const toc = ${serializeJs(toc ?? [])};`,
    '</script>',
    '<script setup lang="ts">',
    css ? 'import "virtual:mog-arborium.css";' : null,
    addDocumentCssImport(embedCss, filePath),
    addEmbedImports(embedComponents, filePath),
    embedComponents.length > 0
      ? `const htmlParts = ${JSON.stringify(htmlParts)};`
      : `const htmlContent = ${JSON.stringify(htmlParts.join(''))};`,
    '',
    'defineExpose({ metadata, toc });',
    '</script>',
    '',
    '<template>',
    `  ${templateContent}`,
    '</template>'
  );
}
