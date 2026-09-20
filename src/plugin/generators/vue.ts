import type { MogParseResult } from '@parser';
import {
  addDocumentCssImport,
  addEmbedImports,
  classAttr,
  htmlSlots,
  joinSegments,
  lines,
  serializeJs,
  writeSegments,
} from './helpers.js';

export function generateVue(
  { segments, metadata, toc, embedComponents = [], embedCss = '' }: MogParseResult,
  css: string,
  filePath?: string
): string {
  const hasEmbeds = embedComponents.length > 0;
  const template = hasEmbeds
    ? [
        '<div>',
        ...writeSegments(segments, {
          html: (_html, slot) => `<div style="display: contents" v-html="html[${slot}]"></div>`,
          embed: i => `<Embed${i} />`,
          open: (tag, classes) => `<${tag}${classAttr(classes)}>`,
          close: tag => `</${tag}>`,
          leaf: (tag, classes, _html, slot) =>
            `<${tag}${classAttr(classes)} v-html="html[${slot}]"></${tag}>`,
        }).map(line => `  ${line}`),
        '</div>',
      ]
    : ['<div v-html="html"></div>'];

  return lines(
    '<script lang="ts">',
    `export const metadata = ${serializeJs(metadata ?? {})};`,
    `export const toc = ${serializeJs(toc ?? [])};`,
    '</script>',
    '<script setup lang="ts">',
    css ? 'import "virtual:mog-arborium.css";' : null,
    addDocumentCssImport(embedCss, filePath),
    addEmbedImports(embedComponents, filePath),
    `const html = ${JSON.stringify(hasEmbeds ? htmlSlots(segments) : joinSegments(segments, () => ''))};`,
    '',
    'defineExpose({ metadata, toc });',
    '</script>',
    '',
    '<template>',
    template.map(line => `  ${line}`),
    '</template>'
  );
}
