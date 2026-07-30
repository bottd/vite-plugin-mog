import { readFile, readdir } from 'node:fs/promises';
import { resolve, dirname, basename, isAbsolute } from 'node:path';
import {
  createFilter,
  normalizePath,
  transformWithOxc,
  type EnvironmentModuleGraph,
  type EnvironmentModuleNode,
  type FilterPattern,
  type Plugin,
} from 'vite';
import { parseMog, getThemeCss, themeNames, OutputMode } from '@parser';
import { generateOutput, type GeneratorMode } from './generators/index.js';

export interface MogPluginOptions {
  mode: GeneratorMode;
  include?: FilterPattern;
  exclude?: FilterPattern;
  /**
   * Syntax highlighting theme for verbatim blocks. A pair emits both behind
   * `prefers-color-scheme`; omitting it leaves code unstyled.
   */
  theme?: string | { light: string; dark: string };
  componentDir?: string;
  components?: Record<string, string>;
}

const VIRTUAL_CSS_ID = 'virtual:mog-arborium.css';
const RESOLVED_VIRTUAL_CSS_ID = `\0${VIRTUAL_CSS_ID}`;

const VIRTUAL_DOC_CSS_PREFIX = 'virtual:mog-css:';
const RESOLVED_VIRTUAL_DOC_CSS_PREFIX = `\0${VIRTUAL_DOC_CSS_PREFIX}`;

const modeExtensions: Record<GeneratorMode, string | null> = {
  [OutputMode.html]: null,
  [OutputMode.svelte]: '.svelte',
  [OutputMode.vue]: '.vue',
  [OutputMode.react]: '.jsx',
  metadata: null,
};

function buildCss(theme?: MogPluginOptions['theme']): string {
  if (!theme) return '';
  if (typeof theme === 'string') return themeCss(theme);

  return `
    @media (prefers-color-scheme: light) {\n${themeCss(theme.light)}\n}
    @media (prefers-color-scheme: dark) {\n${themeCss(theme.dark)}\n}
  `;
}

function themeCss(theme: string): string {
  const css = getThemeCss(theme);
  if (!css) {
    throw new Error(
      `[vite-plugin-mog] Unknown Arborium theme ${JSON.stringify(theme)}. ` +
        `Available themes: ${themeNames().join(', ')}.`
    );
  }
  return css;
}

async function scanComponentDir(dir: string, mode: GeneratorMode): Promise<Map<string, string>> {
  const ext = modeExtensions[mode];
  if (!ext) return new Map();

  const extensions = mode === OutputMode.react ? ['.jsx', '.tsx'] : [ext];
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

  const components = new Map<string, string>();
  for (const entry of entries) {
    // A symlinked component reports as a link rather than a file, and pnpm
    // workspaces are full of them.
    const isFile = entry.isFile() || entry.isSymbolicLink();
    const extension = isFile && extensions.find(ext => entry.name.endsWith(ext));
    if (!extension) continue;

    const path = normalizePath(resolve(entry.parentPath, entry.name));
    const name = basename(entry.name, extension);
    validateComponentName(name);
    const duplicate = components.get(name);
    if (duplicate) {
      throw new Error(
        `[vite-plugin-mog] Duplicate component name ${JSON.stringify(name)}: ` +
          `${duplicate} and ${path}.`
      );
    }
    components.set(name, path);
  }
  return components;
}

function validateComponentName(name: string): void {
  if (!/^[A-Za-z_$][A-Za-z0-9_$]*$/.test(name)) {
    throw new Error(
      `[vite-plugin-mog] Component name ${JSON.stringify(name)} is not a valid JavaScript identifier.`
    );
  }
}

function componentImportPath(importPath: string, root: string): string {
  if (importPath.startsWith('.') || isAbsolute(importPath)) {
    return normalizePath(resolve(root, importPath));
  }
  return importPath;
}

function cleanModuleId(id: string): string {
  const withoutVirtualPrefix = id.startsWith('\0') ? id.slice(1) : id;
  return withoutVirtualPrefix.split('?', 1)[0];
}

function injectAfterTag(code: string, pattern: RegExp, content: string): string | null {
  const match = code.match(pattern);
  if (!match || match.index === undefined) return null;
  const pos = match.index + match[0].length;
  return code.slice(0, pos) + '\n' + content + '\n' + code.slice(pos);
}

function injectComponentImports(
  code: string,
  components: Map<string, string>,
  mode: GeneratorMode
): string {
  if (components.size === 0) return code;

  const imports = [...components]
    .map(([name, path]) => `import ${name} from ${JSON.stringify(path)};`)
    .join('\n');

  if (mode === OutputMode.svelte) {
    return (
      injectAfterTag(code, /<script(?![^>]*\b(?:module|context\s*=))[^>]*>/, imports) ??
      `<script>\n${imports}\n</script>\n${code}`
    );
  }

  if (mode === OutputMode.vue) {
    const wrapped = /<template[\s>]/.test(code) ? code : `<template>\n${code}\n</template>`;
    return (
      injectAfterTag(wrapped, /<script\s+setup[^>]*>/, imports) ??
      `<script setup>\n${imports}\n</script>\n${wrapped}`
    );
  }

  if (mode === OutputMode.react) {
    return imports + '\n' + code;
  }

  return code;
}

/** Whether vite-plugin-svelte is set up to compile `.mg` files itself. */
function svelteCompilesMog(plugins: readonly Plugin[]): boolean {
  const config = plugins.find(plugin => plugin.name === 'vite-plugin-svelte:config');
  return config?.api?.options?.extensions?.includes('.mg') ?? false;
}

export function mogPlugin(options: MogPluginOptions): Plugin {
  const { include, exclude, mode, theme, componentDir, components: explicitComponents } = options;

  if (!Object.prototype.hasOwnProperty.call(modeExtensions, mode)) {
    throw new Error(
      `[vite-plugin-mog] Invalid mode ${JSON.stringify(mode)}. ` +
        `Expected one of: ${Object.keys(modeExtensions).join(', ')}.`
    );
  }
  const filter = createFilter(include, exclude);
  const css = buildCss(theme);
  const ext = modeExtensions[mode];
  // Whether a resolved `.mg` id carries `ext` — see configResolved.
  let appendExt = true;

  type ParseResult = Awaited<ReturnType<typeof parseMog>>;
  const parseCache = new Map<string, Promise<ParseResult>>();
  const embedModules = new Map<string, { basePath: string; index: number }>();
  // Until configResolved lands, the cwd is the best guess at the project root.
  let root = normalizePath(process.cwd());
  let components = new Map<string, string>();

  function resolvedComponentDir(): string | undefined {
    return componentDir ? normalizePath(resolve(root, componentDir)) : undefined;
  }

  async function refreshComponents(): Promise<void> {
    const dir = resolvedComponentDir();
    const refreshed = dir ? await scanComponentDir(dir, mode) : new Map<string, string>();
    for (const [name, importPath] of Object.entries(explicitComponents ?? {})) {
      validateComponentName(name);
      refreshed.set(name, componentImportPath(importPath, root));
    }
    components = refreshed;
  }

  function parseCacheKey(filePath: string, parserMode?: string): string {
    return `${filePath}\0${parserMode ?? ''}`;
  }

  function invalidateParse(filePath: string): void {
    const prefix = `${normalizePath(filePath)}\0`;
    for (const key of parseCache.keys()) {
      if (key.startsWith(prefix)) parseCache.delete(key);
    }
  }

  function cachedParse(
    filePath: string,
    parserMode: string | undefined,
    warn: (message: string) => void
  ): Promise<ParseResult> {
    const key = parseCacheKey(filePath, parserMode);
    let pending = parseCache.get(key);
    if (!pending) {
      const fresh: Promise<ParseResult> = readFile(filePath, 'utf-8')
        .then(async content => {
          const result = await parseMog(content, parserMode);
          if (parseCache.get(key) !== fresh) return cachedParse(filePath, parserMode, warn);
          result.diagnostics?.forEach(warn);
          return result;
        })
        .catch(error => {
          if (parseCache.get(key) !== fresh) return cachedParse(filePath, parserMode, warn);
          throw error;
        });
      pending = fresh;
      parseCache.set(key, fresh);
      void fresh.catch(() => {
        if (parseCache.get(key) === fresh) parseCache.delete(key);
      });
    }
    return pending;
  }

  function invalidateModules(
    moduleGraph: EnvironmentModuleGraph,
    moduleIds: Iterable<string>
  ): EnvironmentModuleNode[] {
    const modules: EnvironmentModuleNode[] = [];
    for (const id of moduleIds) {
      const mod = moduleGraph.getModuleById(id);
      if (mod) {
        moduleGraph.invalidateModule(mod);
        modules.push(mod);
      }
    }
    return modules;
  }

  return {
    name: 'vite-plugin-mog',
    enforce: 'pre',

    configResolved: {
      order: 'post',
      handler(config) {
        root = normalizePath(config.root);

        // SvelteKit looks a route up in the Vite manifest by its path
        if (mode === OutputMode.svelte && svelteCompilesMog(config.plugins)) {
          appendExt = false;
        }
      },
    },

    async buildStart() {
      const dir = resolvedComponentDir();
      if (dir) this.addWatchFile(dir);
      await refreshComponents();
    },

    configureServer(server) {
      const dir = resolvedComponentDir();
      if (dir) server.watcher.add(dir);
    },

    async resolveId(id: string, importer?: string) {
      if (id === VIRTUAL_CSS_ID) {
        return RESOLVED_VIRTUAL_CSS_ID;
      }

      if (id.startsWith(VIRTUAL_DOC_CSS_PREFIX)) {
        return `\0${id}.css`;
      }

      if (ext && appendExt && id.endsWith('.mg')) {
        // No importer means a build entry; it resolves against the project root.
        const cleanImporter = importer ? cleanModuleId(importer) : undefined;
        // A generated module declares its Mog source as a watch dependency, and
        // Vite resolves that back through here. Rewriting it would loop.
        const isOwnWatchDependency =
          cleanImporter !== undefined &&
          isAbsolute(id) &&
          (cleanImporter.endsWith(`.mg${ext}`) || cleanImporter.startsWith(VIRTUAL_DOC_CSS_PREFIX));
        if (isOwnWatchDependency) return;

        const resolved = await this.resolve?.(id, cleanImporter, {
          skipSelf: true,
        });
        const basePath = normalizePath(
          cleanModuleId(resolved?.id ?? resolve(cleanImporter ? dirname(cleanImporter) : root, id))
        );
        if (filter(basePath)) {
          // Deliberately not `\0`-prefixed: Vite's createFilter rejects every id
          // containing a NUL, and @vitejs/plugin-vue gates its transform on it,
          // so a virtual `\0…doc.mg.vue` never reaches the Vue compiler.
          return `${basePath}${ext}`;
        }
      }

      if (ext && id.includes('.mg?embed=') && importer) {
        const [relativePath, query] = id.split('?', 2);
        const cleanImporter = cleanModuleId(importer);
        const resolved = isAbsolute(relativePath)
          ? undefined
          : await this.resolve?.(relativePath, cleanImporter, {
              skipSelf: true,
            });
        const basePath = normalizePath(
          cleanModuleId(resolved?.id ?? resolve(dirname(cleanImporter), relativePath))
        );
        const index = parseInt(new URLSearchParams(query).get('embed') ?? '', 10);
        if (Number.isNaN(index)) return;
        const resolvedId = `${basePath}${appendExt ? ext : ''}?${query}`;
        embedModules.set(resolvedId, { basePath, index });
        return resolvedId;
      }
    },

    async load(id: string) {
      const parse = (filePath: string, parserMode: string | undefined) =>
        cachedParse(filePath, parserMode, message => this.warn({ id: filePath, message }));
      const watch = (filePath: string) => this.addWatchFile?.(filePath);
      const parserMode = mode === 'metadata' ? undefined : mode;

      if (id === RESOLVED_VIRTUAL_CSS_ID) {
        return css;
      }

      if (id.startsWith(RESOLVED_VIRTUAL_DOC_CSS_PREFIX) && id.endsWith('.css')) {
        const filePath = normalizePath(id.slice(RESOLVED_VIRTUAL_DOC_CSS_PREFIX.length, -4));
        watch(filePath);
        const result = await parse(filePath, parserMode);
        return result.embedCss ?? '';
      }

      const embedInfo = embedModules.get(id);
      if (embedInfo) {
        const { basePath, index } = embedInfo;
        watch(basePath);
        const result = await parse(basePath, parserMode);

        const embed = result.embedComponents?.[index];
        if (!embed) {
          throw new Error(`Embed component ${index} not found in ${basePath}`);
        }

        let code = embed.code;
        if (mode === OutputMode.react) {
          code = `export default function MogEmbed() { return <>${code}</>; }`;
        }

        return injectComponentImports(code, components, mode);
      }

      const rawId = id.startsWith('\0') ? id.slice(1) : id;
      const [idWithoutQuery, query] = rawId.split('?', 2);
      if (query && query !== 'metadata') return;
      let basePath = normalizePath(idWithoutQuery);
      if (ext && idWithoutQuery.endsWith(`.mg${ext}`)) {
        basePath = idWithoutQuery.slice(0, -ext.length);
      }
      if (!basePath.endsWith('.mg') || !filter(basePath)) return;

      const outputMode: GeneratorMode = query === 'metadata' ? 'metadata' : mode;

      try {
        watch(basePath);
        const result = await parse(basePath, outputMode === 'metadata' ? undefined : parserMode);
        return generateOutput(outputMode, result, css, basePath);
      } catch (error) {
        this.error(`Failed to parse mog file ${basePath}: ${error}`);
      }
    },

    async transform(code, id) {
      if (mode !== OutputMode.react) return;
      if (!ext || !id.includes(`.mg${ext}`)) return;
      return transformWithOxc(code, id, {
        lang: ext.slice(1) as 'jsx',
        jsx: { runtime: 'automatic' },
      });
    },

    // `build --watch` never runs hotUpdate, and a dev server runs both — so the
    // overlap here is deliberate. invalidateParse is idempotent.
    watchChange(id) {
      if (id.endsWith('.mg')) invalidateParse(id);
    },

    async hotUpdate(ctx) {
      const file = normalizePath(ctx.file);
      const dir = resolvedComponentDir();
      if (dir && file.startsWith(dir + '/')) {
        await refreshComponents();

        const invalidated = invalidateModules(this.environment.moduleGraph, embedModules.keys());
        if (invalidated.length > 0) {
          return [...new Set([...ctx.modules, ...invalidated])];
        }
        return;
      }

      if (file.endsWith('.mg')) invalidateParse(file);
    },
  } satisfies Plugin;
}
