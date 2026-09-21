import type { MogParseResult } from '@parser';
import {
  addDocumentCssImport,
  addEmbedImports,
  lines,
  serializeJs,
  soleHtml,
  writeSegments,
} from './helpers.js';

const escapeAttr = (value: string) =>
  value.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');
const quotedAttr = (name: string, value: string) => `${name}="${escapeAttr(value)}"`;

export function generateVue(
  { segments, metadata, toc, embedComponents = [], embedCss = '' }: MogParseResult,
  css: string,
  filePath?: string
): string {
  // The strings ship as one array the template indexes, rather than being
  // escaped into `v-html` attributes one by one.
  const html: string[] = [];
  const slot = (content: string) => `v-html="html[${html.push(content) - 1}]"`;
  // A document that is a single string needs no wrapper: the root can hold it.
  const sole = soleHtml(segments);
  const template =
    sole !== undefined
      ? [`<div ${slot(sole)}></div>`]
      : [
          '<div>',
          ...writeSegments(segments, {
            html: content => `<div style="display: contents" ${slot(content)}></div>`,
            leaf: (tag, attrs, content) => `<${tag}${attrs} ${slot(content)}></${tag}>`,
            attr: quotedAttr,
          }).map(line => `  ${line}`),
          '</div>',
        ];

  return lines(
    '<script lang="ts">',
    `export const metadata = ${serializeJs(metadata ?? {})};`,
    `export const toc = ${serializeJs(toc ?? [])};`,
    '</script>',
    '<script setup lang="ts">',
    css ? 'import "virtual:mog-arborium.css";' : null,
    addDocumentCssImport(embedCss, filePath),
    addEmbedImports(embedComponents, filePath),
    `const html = ${JSON.stringify(html)};`,
    '',
    'defineExpose({ metadata, toc });',
    '</script>',
    '',
    '<template>',
    template.map(line => `  ${line}`),
    '</template>'
  );
}
