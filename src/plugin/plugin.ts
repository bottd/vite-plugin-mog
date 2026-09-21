import { resolve, dirname, isAbsolute } from 'node:path';
import {
  createFilter,
  normalizePath,
  transformWithOxc,
  type EnvironmentModuleGraph,
  type EnvironmentModuleNode,
  type FilterPattern,
  type Plugin,
} from 'vite';
import {
  getThemeCss,
  themeNames,
  OutputMode,
  DataAttributesMode,
  type DataAttributes,
} from '@parser';
import { generateOutput, type GeneratorMode } from './generators/index.js';
import { generateMetadata } from './generators/metadata.js';
import { discoverComponents, injectComponentImports, modeExtensions } from './components.js';
import { createParseCache } from './parse-cache.js';

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
  /**
   * Which of a node's `attr` keys render as `data-*` attributes on its element.
   *
   * `false` (the default) renders none, `true` renders all, and an array selects
   * by top-level key. Data in a document is not necessarily data for the DOM,
   * and shipping it to every visitor by default is the surprising direction:
   * documents that carry numbers for a build pipeline paid for them in page
   * weight without anything reading them.
   *
   * Root-level `attr` blocks are unaffected — they remain `metadata`.
   */
  dataAttributes?: boolean | string[];
}

function dataAttributes(option: MogPluginOptions['dataAttributes']): DataAttributes {
  if (Array.isArray(option)) {
    return { mode: DataAttributesMode.allow, keys: option };
  }
  return { mode: option ? DataAttributesMode.all : DataAttributesMode.none, keys: undefined };
}

const VIRTUAL_CSS_ID = 'virtual:mog-arborium.css';
const RESOLVED_VIRTUAL_CSS_ID = `\0${VIRTUAL_CSS_ID}`;

const VIRTUAL_DOC_CSS_PREFIX = 'virtual:mog-css:';
const RESOLVED_VIRTUAL_DOC_CSS_PREFIX = `\0${VIRTUAL_DOC_CSS_PREFIX}`;

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

function cleanModuleId(id: string): string {
  const withoutVirtualPrefix = id.startsWith('\0') ? id.slice(1) : id;
  return withoutVirtualPrefix.split('?', 1)[0];
}

/** Whether vite-plugin-svelte is set up to compile `.mg` files itself. */
function svelteCompilesMog(plugins: readonly Plugin[]): boolean {
  const config = plugins.find(plugin => plugin.name === 'vite-plugin-svelte:config');
  return config?.api?.options?.extensions?.includes('.mg') ?? false;
}

export function mogPlugin(options: MogPluginOptions): Plugin {
  const {
    include,
    exclude,
    mode,
    theme,
    componentDir,
    components: explicitComponents,
    dataAttributes: dataAttributesOption,
  } = options;
  const data = dataAttributes(dataAttributesOption);

  if (!Object.hasOwn(modeExtensions, mode)) {
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

  const parseCache = createParseCache(mode === 'metadata' ? undefined : mode, data);
  const embedModules = new Map<string, { basePath: string; index: number }>();
  // Until configResolved lands, the cwd is the best guess at the project root.
  let root = normalizePath(process.cwd());
  let components = new Map<string, string>();

  function resolvedComponentDir(): string | undefined {
    return componentDir ? normalizePath(resolve(root, componentDir)) : undefined;
  }

  async function refreshComponents(): Promise<void> {
    components = await discoverComponents(root, resolvedComponentDir(), mode, explicitComponents);
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

    // `scan` is set by the dep scanner but missing from Vite's public hook type.
    async resolveId(
      id: string,
      importer: string | undefined,
      options?: { isEntry: boolean; scan?: boolean }
    ) {
      // Ahead of the scan guard: user code imports these directly, and the `\0`
      // keeps them external to the scanner.
      if (id === VIRTUAL_CSS_ID) {
        return RESOLVED_VIRTUAL_CSS_ID;
      }

      if (id.startsWith(VIRTUAL_DOC_CSS_PREFIX)) {
        return `\0${id}.css`;
      }

      // The dep scanner loads from disk, so the ids below would be files it
      // cannot read. Unrewritten, `.mg` is skipped as unscannable.
      if (options?.scan) return;

      // A generated id can be imported by its own name — plugin-react's HMR
      // preamble does exactly that — and only this plugin can resolve it.
      if (ext && id.includes(`.mg${ext}`)) {
        const basePath = cleanModuleId(id).slice(0, -ext.length);
        if (basePath.endsWith('.mg') && isAbsolute(basePath) && filter(basePath)) return id;
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
      const warn = (filePath: string) => (message: string) => this.warn({ id: filePath, message });
      const watch = (filePath: string) => this.addWatchFile?.(filePath);

      if (id === RESOLVED_VIRTUAL_CSS_ID) {
        return css;
      }

      if (id.startsWith(RESOLVED_VIRTUAL_DOC_CSS_PREFIX) && id.endsWith('.css')) {
        const filePath = normalizePath(id.slice(RESOLVED_VIRTUAL_DOC_CSS_PREFIX.length, -4));
        watch(filePath);
        const result = await parseCache.render(filePath, warn(filePath));
        return result.embedCss ?? '';
      }

      const embedInfo = embedModules.get(id);
      if (embedInfo) {
        const { basePath, index } = embedInfo;
        watch(basePath);
        const result = await parseCache.render(basePath, warn(basePath));

        const embed = result.embedComponents?.[index];
        if (!embed) {
          throw new Error(`Embed component ${index} not found in ${basePath}`);
        }

        let code = embed.code;
        if (mode === OutputMode.react) {
          code = `export default function MogEmbed() { return <>${code}</>; }`;
        }

        return injectComponentImports(code, components, mode, basePath);
      }

      const idWithoutQuery = cleanModuleId(id);
      const query = id.split('?', 2)[1];
      if (query && query !== 'metadata') return;
      let basePath = normalizePath(idWithoutQuery);
      if (ext && idWithoutQuery.endsWith(`.mg${ext}`)) {
        basePath = idWithoutQuery.slice(0, -ext.length);
      }
      if (!basePath.endsWith('.mg') || !filter(basePath)) return;

      const outputMode: GeneratorMode = query === 'metadata' ? 'metadata' : mode;

      try {
        watch(basePath);
        if (outputMode === 'metadata') {
          return generateMetadata(await parseCache.metadata(basePath, warn(basePath)));
        }
        const result = await parseCache.render(basePath, warn(basePath));
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
    // overlap here is deliberate. Invalidating a revision is idempotent.
    watchChange(id) {
      if (id.endsWith('.mg')) parseCache.invalidate(id);
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

      if (file.endsWith('.mg')) parseCache.invalidate(file);
    },
  } satisfies Plugin;
}
