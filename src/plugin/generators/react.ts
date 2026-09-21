import type { MogParseResult } from '@parser';
import {
  addDocumentCssImport,
  addEmbedImports,
  attrExpr,
  lines,
  writeSegments,
} from './helpers.js';

const innerHtml = (html: string) => `dangerouslySetInnerHTML={{ __html: ${JSON.stringify(html)} }}`;

export function generateReact(
  { segments, metadata, toc, embedComponents = [], embedCss = '' }: MogParseResult,
  css: string,
  filePath?: string
): string {
  const children = writeSegments(segments, {
    html: html => `<div style={{ display: 'contents' }} ${innerHtml(html)} />`,
    leaf: (tag, attrs, html) => `<${tag}${attrs} ${innerHtml(html)} />`,
    attr: attrExpr,
    className: 'className',
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
