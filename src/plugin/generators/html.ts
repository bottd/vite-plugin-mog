import type { EmbedComponent, MogParseResult } from '../types/parser.js';
import { lines } from './helpers.js';

function mergeEmbeds(htmlParts: string[], embeds: EmbedComponent[]): string {
  return htmlParts
    .flatMap((part, i) => (i < embeds.length ? [part, embeds[i].code] : [part]))
    .join('');
}

export function generateHtml(
  { htmlParts, metadata, toc, embedComponents = [], embedCss = '' }: MogParseResult,
  css: string
): string {
  const raw = mergeEmbeds(htmlParts, embedComponents);
  // The framework modes import `virtual:mog-css:` and let Vite own the CSS.
  // html mode inlines a <style> instead, on purpose: the `html` export is
  // routinely written straight to a file, and styles have to travel with it.
  // The trade is that document CSS skips Vite's CSS pipeline here.
  const html = embedCss ? `<style>${embedCss}</style>${raw}` : raw;
  return lines(
    css ? 'import "virtual:mog-arborium.css";' : null,
    '',
    `export const metadata = ${JSON.stringify(metadata ?? {})};`,
    `export const html = ${JSON.stringify(html)};`,
    `export const toc = ${JSON.stringify(toc ?? [])};`,
    '',
    'export default { metadata, html, toc };'
  );
}
