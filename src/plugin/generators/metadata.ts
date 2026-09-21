import type { MogMetadataResult } from '@parser';
import { lines } from './helpers.js';

export function generateMetadata({
  metadata,
  toc,
}: Pick<MogMetadataResult, 'metadata' | 'toc'>): string {
  return lines(
    `export const metadata = ${JSON.stringify(metadata ?? {})};`,
    `export const toc = ${JSON.stringify(toc ?? [])};`,
    'export default { metadata, toc };'
  );
}
