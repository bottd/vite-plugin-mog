import type { MogParseResult } from '@parser';
import { addDocumentCssImport, addEmbedImports, lines, writeSegments } from './helpers.js';

const className = (classes: string) => (classes ? ` className={${JSON.stringify(classes)}}` : '');
const innerHtml = (html: string) => `dangerouslySetInnerHTML={{ __html: ${JSON.stringify(html)} }}`;

export function generateReact(
  { segments, metadata, toc, embedComponents = [], embedCss = '' }: MogParseResult,
  css: string,
  filePath?: string
): string {
  const children = writeSegments(segments, {
    html: html => `<div style={{ display: 'contents' }} ${innerHtml(html)} />`,
    embed: i => `<Embed${i} />`,
    open: (tag, classes) => `<${tag}${className(classes)}>`,
    close: tag => `</${tag}>`,
    leaf: (tag, classes, html) => `<${tag}${className(classes)} ${innerHtml(html)} />`,
  });

  return lines(
    css ? 'import "virtual:mog-arborium.css";' : null,
    addDocumentCssImport(embedCss, filePath),
    addEmbedImports(embedComponents, filePath),
    '',
    `export const metadata = ${JSON.stringify(metadata ?? {})};`,
    `export const toc = ${JSON.stringify(toc ?? [])};`,
    '',
    'export function Component() {',
    '  return (',
    '    <>',
    children.map(line => `      ${line}`),
    '    </>',
    '  );',
    '}',
    'export default Component;'
  );
}
