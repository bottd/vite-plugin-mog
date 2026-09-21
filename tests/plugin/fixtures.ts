import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import ts from 'typescript';
import type { MogParseResult } from '@parser';

export const fixturesDir = join(import.meta.dirname, '../fixtures');
export const componentsDir = join(fixturesDir, 'components');

// Every fixture each generator can render. The framework-specific embed
// fixtures need wiring that lives in embed-output.test.ts, and diagnostics.mg
// exists to exercise warnings rather than clean output.
const notRenderable = new Set([
  'diagnostics.mg',
  'diagnostics-repeated.mg',
  'embed.mg',
  'embed-html.mg',
  'embed-react.mg',
  'embed-vue.mg',
]);

export const fixtures = readdirSync(fixturesDir).filter(
  name => name.endsWith('.mg') && !notRenderable.has(name)
);

/** An otherwise-empty parse result, for driving a generator directly. */
export function parseResult(overrides: Partial<MogParseResult> = {}): MogParseResult {
  return {
    metadata: {},
    segments: [],
    toc: [],
    embedComponents: [],
    embedCss: '',
    ...overrides,
  };
}

/** Type errors a consumer file produces against a declaration, as messages. */
export function typeCheck(
  declaration: string,
  consumer: string,
  options: ts.CompilerOptions = {}
): string[] {
  const program = ts.createProgram([declaration, consumer], {
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    target: ts.ScriptTarget.ES2020,
    noEmit: true,
    skipLibCheck: true,
    strict: true,
    ...options,
  });

  return ts
    .getPreEmitDiagnostics(program)
    .map(diagnostic => ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n'));
}

export async function loadCode(
  plugin: { load?: (id: string) => unknown | Promise<unknown> },
  id: string
): Promise<string | undefined> {
  const result = await plugin.load?.(id);
  if (result == null) return undefined;
  return typeof result === 'string' ? result : (result as { code: string }).code;
}
