/**
 * Whether the JavaScript and the native binary came from the same tree, and the
 * message to throw when they did not.
 *
 * Comparing versions cannot answer this. `files` excludes `dist/napi/*.node`,
 * so installing a local checkout with `file:` copies the new JavaScript and no
 * binary, and the loader falls back to the published platform package already in
 * the store. Both halves then report the same version while the binary is an
 * older parser. The release path has the mirror-image problem: the platform
 * binaries are built before the version bump, so a published binary always
 * carries the previous version string. A digest of the sources both halves were
 * built from is the thing that actually differs.
 *
 * `locate` and `packageVersion` are functions rather than strings: finding the
 * binary and reading package.json are only worth doing when there is an error to
 * write, and this runs on every import.
 */
export function buildMismatch({ expected, actual, packageVersion, binaryVersion, locate }) {
  if (actual === expected) return null;

  return (
    `[vite-plugin-mog] The native binary was not built from this JavaScript.\n` +
    `  package ${packageVersion()} (build ${expected})\n` +
    `  binary  ${binaryVersion} (build ${actual})\n` +
    `  loaded from ${locate()}\n` +
    `Run \`pnpm run build\` to rebuild both halves. If you installed a local ` +
    `checkout, use \`link:\` rather than \`file:\` — \`file:\` copies the ` +
    `JavaScript without the binary, and the loader silently falls back to the ` +
    `published one.`
  );
}
