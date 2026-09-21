import { readFileSync, readdirSync } from 'node:fs';
import { join, relative } from 'node:path';

/**
 * The digest `build.rs` stamps into the native binary, recomputed here.
 *
 * It must agree byte for byte with the Rust side: same files, same order, same
 * hash. FNV-1a because it has to be reimplementable in two languages and stable
 * across six build hosts — not because anything here is adversarial.
 */
const OFFSET = 0xcbf29ce484222325n;
const PRIME = 0x100000001b3n;
const MASK = 0xffffffffffffffffn;

function digest(hash, bytes) {
  for (const byte of bytes) {
    hash = ((hash ^ BigInt(byte)) * PRIME) & MASK;
  }
  return hash;
}

function sources(dir, root) {
  return readdirSync(join(root, dir), { recursive: true, withFileTypes: true })
    .filter(entry => !entry.isDirectory() && entry.name.endsWith('.rs'))
    .map(entry => relative(root, join(entry.parentPath, entry.name)).replaceAll('\\', '/'));
}

export function buildId(root) {
  const files = [...sources('src/parser', root), 'Cargo.lock'].sort();

  let hash = OFFSET;
  for (const path of files) {
    hash = digest(hash, Buffer.from(path, 'utf8'));
    hash = digest(hash, readFileSync(join(root, path)));
  }
  return hash.toString(16).padStart(16, '0');
}
