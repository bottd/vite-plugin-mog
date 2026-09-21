import type { MogParseResult } from '@parser';
import { lines, soleHtml } from './helpers.js';

export function generateHtml(
  { segments, metadata, toc, embedCss = '' }: MogParseResult,
  css: string
): string {
  // An html embed is markup already, so the parser writes it into the HTML where
  // it stands and lifts nothing. Anything else here was parsed in another mode.
  const raw = soleHtml(segments);
  if (raw === undefined) throw new Error('html mode cannot mount component embeds');
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
