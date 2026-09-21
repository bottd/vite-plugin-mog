// Outside the flake's dev shell there is no rustc on PATH, and the failure
// surfaces as a Node stack trace from @napi-rs/cli with the real cause on the
// first line, where it is easy to miss.

import { spawnSync } from 'node:child_process';

const missing = ['rustc', 'cargo'].filter(
  tool => spawnSync(tool, ['--version'], { stdio: 'ignore' }).status !== 0
);

if (missing.length > 0) {
  console.error(
    `[vite-plugin-mog] ${missing.join(' and ')} not found on PATH. ` +
      `Build with: nix develop --command pnpm run build`
  );
  process.exit(1);
}
