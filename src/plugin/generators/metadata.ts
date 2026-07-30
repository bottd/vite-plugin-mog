import type { MogParseResult } from '../types/parser.js';
import { lines } from './helpers.js';

export function generateMetadata({ metadata, toc }: MogParseResult): string {
  return lines(
    `export const metadata = ${JSON.stringify(metadata ?? {})};`,
    `export const toc = ${JSON.stringify(toc ?? [])};`,
    'export default { metadata, toc };'
  );
}
