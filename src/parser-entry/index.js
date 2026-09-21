// The public parser entry: `vite-plugin-mog/parser`.
//
// Plain JavaScript beside a hand-written `.d.ts`, copied into `dist/parser`
// rather than bundled. It imports the generated binding by relative path, which
// no bundler alias has to rewrite, and it pulls in nothing from Vite or any
// framework peer — a build script can depend on this package alone.

import { createRequire } from 'node:module';
import { readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import {
  buildId,
  version,
  parseMog,
  parseMogMetadata,
  parseMogAstJson,
  getThemeCss,
  themeNames,
  OutputMode,
  DataAttributesMode,
} from '../napi/index.js';

import { BUILD_ID } from './build-id.js';
import { buildMismatch } from './check.js';

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));

const packageJson = () => require('../../package.json');

// Where the loader would have found the binary. Only ever called to write an
// error message, so it can afford to go looking.
function binaryLocation() {
  const napi = join(here, '..', 'napi');
  try {
    const local = readdirSync(napi).find(name => name.endsWith('.node'));
    if (local) return join(napi, local);
  } catch {
    // no dist/napi at all; fall through to the platform packages
  }

  const { optionalDependencies = {} } = packageJson();
  for (const name of Object.keys(optionalDependencies)) {
    try {
      return require.resolve(name);
    } catch {
      continue;
    }
  }
  return 'unknown (no native binary resolved)';
}

const mismatch = buildMismatch({
  expected: BUILD_ID,
  actual: buildId(),
  packageVersion: () => packageJson().version,
  binaryVersion: version(),
  locate: binaryLocation,
});
if (mismatch) throw new Error(mismatch);

/**
 * The document as a tree, for callers that want the source rather than HTML.
 *
 * The binding hands the tree over as JSON: building the object graph through
 * napi would cost several calls per node on the JS thread, which is slower than
 * `JSON.parse` and would serialise every concurrent parse.
 */
export async function parseMogAst(content, options) {
  return JSON.parse(await parseMogAstJson(content, options));
}

export {
  parseMog,
  parseMogMetadata,
  getThemeCss,
  themeNames,
  OutputMode,
  DataAttributesMode,
  version,
  buildId,
};
