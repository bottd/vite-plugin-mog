import { readdir } from 'node:fs/promises';
import { basename, isAbsolute, resolve } from 'node:path';
import { normalizePath } from 'vite';
import type { GeneratorMode } from './generators/index.js';

export const modeExtensions: Record<GeneratorMode, string | null> = {
  html: null,
  svelte: '.svelte',
  vue: '.vue',
  react: '.jsx',
  metadata: null,
};

function validateName(name: string): void {
  if (!/^[A-Za-z_$][A-Za-z0-9_$]*$/.test(name)) {
    throw new Error(
      `[vite-plugin-mog] Component name ${JSON.stringify(name)} is not a valid JavaScript identifier.`
    );
  }
}

export async function discoverComponents(
  root: string,
  dir: string | undefined,
  mode: GeneratorMode,
  explicit: Record<string, string> = {}
): Promise<Map<string, string>> {
  const components = new Map<string, string>();
  const ext = modeExtensions[mode];
  if (dir && ext) {
    const extensions = mode === 'react' ? ['.jsx', '.tsx'] : [ext];
    let entries;
    try {
      entries = await readdir(dir, { withFileTypes: true, recursive: true });
    } catch (error) {
      throw new Error(
        `[vite-plugin-mog] Cannot read componentDir ${JSON.stringify(dir)}: ${
          error instanceof Error ? error.message : String(error)
        }`,
        { cause: error }
      );
    }
    for (const entry of entries) {
      const extension =
        (entry.isFile() || entry.isSymbolicLink()) &&
        extensions.find(ext => entry.name.endsWith(ext));
      if (!extension) continue;
      const path = normalizePath(resolve(entry.parentPath, entry.name));
      const name = basename(entry.name, extension);
      validateName(name);
      const duplicate = components.get(name);
      if (duplicate) {
        throw new Error(
          `[vite-plugin-mog] Duplicate component name ${JSON.stringify(name)}: ${duplicate} and ${path}.`
        );
      }
      components.set(name, path);
    }
  }
  for (const [name, path] of Object.entries(explicit)) {
    validateName(name);
    components.set(
      name,
      path.startsWith('.') || isAbsolute(path) ? normalizePath(resolve(root, path)) : path
    );
  }
  return components;
}

export async function injectComponentImports(
  code: string,
  components: Map<string, string>,
  mode: GeneratorMode,
  filename?: string
): Promise<string> {
  if (components.size === 0) return code;
  const imports = [...components]
    .map(([name, path]) => `import ${name} from ${JSON.stringify(path)};`)
    .join('\n');

  if (mode === 'svelte') {
    const { parse, preprocess } = await import('svelte/compiler');
    let ast;
    try {
      // Parse valid Svelte directly: preprocessing's tag scanner cannot tell
      // a real tag from one inside a template expression's string literal.
      ast = parse(code, { filename, modern: true });
    } catch {
      // Preprocessor input may contain other script/style languages. Locate
      // scripts on a tolerant view that preserves the original source offsets.
      const mask = ({ content }: { content: string }) => ({
        code: content.replace(/[^\r\n]/g, ' '),
      });
      const view = await preprocess(code, { script: mask, style: mask }, { filename });
      ast = parse(view.code, { filename, modern: true, loose: true });
    }
    const { instance } = ast;
    if (instance) {
      // Start after parsed attributes so a quoted `>` cannot end the tag.
      const end = instance.attributes.at(-1)?.end ?? instance.start;
      const offset = code.indexOf('>', end) + 1;
      return `${code.slice(0, offset)}\n${imports}\n${code.slice(offset)}`;
    }
    return `<script>\n${imports}\n</script>\n${code}`;
  }

  if (mode === 'vue') {
    const { parse } = await import('vue/compiler-sfc');
    let parsed = parse(code, { filename });
    if (
      !parsed.descriptor.template &&
      !parsed.descriptor.script &&
      !parsed.descriptor.scriptSetup
    ) {
      code = `<template>\n${code}\n</template>`;
      parsed = parse(code, { filename });
    }
    if (parsed.errors.length) throw parsed.errors[0];
    const setup = parsed.descriptor.scriptSetup;
    if (setup) {
      const offset = setup.loc.start.offset;
      return `${code.slice(0, offset)}\n${imports}\n${code.slice(offset)}`;
    }
    // Vue requires both script blocks to agree on language.
    const lang = parsed.descriptor.script?.lang;
    return `<script setup${lang ? ` lang="${lang}"` : ''}>\n${imports}\n</script>\n${code}`;
  }

  return mode === 'react' ? `${imports}\n${code}` : code;
}
