import type { MogParseResult } from '../types/parser.js';
import { addDocumentCssImport, addEmbedImports, lines } from './helpers.js';

export function generateReact(
  { htmlParts, metadata, toc, embedComponents = [], embedCss = '' }: MogParseResult,
  css: string,
  filePath?: string
): string {
  const children = htmlParts
    .flatMap((part, i) => [
      `<div dangerouslySetInnerHTML={{ __html: ${JSON.stringify(part)} }} />`,
      ...(i < embedComponents.length ? [`<Embed${i} />`] : []),
    ])
    .join('\n    ');

  return lines(
    css ? 'import "virtual:mog-arborium.css";' : null,
    addDocumentCssImport(embedCss, filePath),
    addEmbedImports(embedComponents, filePath),
    '',
    `export const metadata = ${JSON.stringify(metadata ?? {})};`,
    `export const toc = ${JSON.stringify(toc ?? [])};`,
    '',
    'export function Component() {',
    `  return <>${children}</>;`,
    '}',
    'export default Component;'
  );
}
