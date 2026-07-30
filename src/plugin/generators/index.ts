import type { OutputMode } from '@parser';
import type { MogParseResult } from '../types/parser.js';
import { generateHtml } from './html.js';
import { generateSvelte } from './svelte.js';
import { generateReact } from './react.js';
import { generateVue } from './vue.js';
import { generateMetadata } from './metadata.js';

/**
 * Every parser output mode, plus `metadata` — which the parser has no notion of
 * because it needs no rendering. Derived from the Rust enum so adding a mode
 * there cannot leave this list behind.
 */
export type GeneratorMode = `${OutputMode}` | 'metadata';

export function generateOutput(
  mode: GeneratorMode,
  result: MogParseResult,
  css: string,
  filePath?: string
): string {
  switch (mode) {
    case 'html':
      return generateHtml(result, css);
    case 'svelte':
      return generateSvelte(result, css, filePath);
    case 'react':
      return generateReact(result, css, filePath);
    case 'vue':
      return generateVue(result, css, filePath);
    case 'metadata':
      return generateMetadata(result);
  }
}
