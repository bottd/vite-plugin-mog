import { readFile } from 'node:fs/promises';
import { normalizePath } from 'vite';
import { parseMog, parseMogMetadata, type DataAttributes } from '@parser';

type Warn = (message: string) => void;
interface Revision {
  source: Promise<string>;
  warnings: Map<string, number>;
}

/** A file revision owns all derived work. Removing it invalidates every mode. */
export function createParseCache(mode: Parameters<typeof parseMog>[1], data: DataAttributes) {
  const files = new Map<string, Revision>();

  function projection<T extends { diagnostics?: string[] }>(
    parse: (content: string) => Promise<T>
  ) {
    // Weak keys let invalidated revisions and their results be collected together.
    const pending = new WeakMap<Revision, Promise<T>>();
    return function load(file: string, warn: Warn): Promise<T> {
      const path = normalizePath(file);
      let revision = files.get(path);
      if (!revision) {
        revision = { source: readFile(path, 'utf8'), warnings: new Map() };
        files.set(path, revision);
      }
      const current = revision;
      const cached = pending.get(current);
      if (cached) return cached;

      const fresh = current.source.then(parse).then(
        result => {
          if (files.get(path) !== current) return load(path, warn);
          const counts = new Map<string, number>();
          for (const message of result.diagnostics ?? []) {
            const count = (counts.get(message) ?? 0) + 1;
            counts.set(message, count);
            if (count <= (current.warnings.get(message) ?? 0)) continue;
            current.warnings.set(message, count);
            warn(message);
          }
          return result;
        },
        error => {
          if (files.get(path) !== current) return load(path, warn);
          throw error;
        }
      );
      pending.set(current, fresh);
      void fresh.catch(() => {
        if (files.get(path) === current) files.delete(path);
      });
      return fresh;
    };
  }

  return {
    render: projection(content => parseMog(content, mode, data)),
    metadata: projection(parseMogMetadata),
    invalidate(file: string) {
      files.delete(normalizePath(file));
    },
  };
}
