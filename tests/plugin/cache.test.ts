const readFile = vi.hoisted(() => vi.fn());

vi.mock('node:fs/promises', async importOriginal => ({
  ...(await importOriginal<typeof import('node:fs/promises')>()),
  readFile,
}));

import { mogPlugin } from '../../src/plugin/index.js';
import { createParseCache } from '../../src/plugin/parse-cache.js';
import { DataAttributesMode } from '@parser';

beforeEach(() => readFile.mockReset());

function cache() {
  return createParseCache('html', { mode: DataAttributesMode.none, keys: undefined });
}

it('shares one source read across concurrent render and metadata projections', async () => {
  readFile.mockResolvedValue(
    '``attr:\ntitle "First"\ntitle "Second"\ntitle "Third"\n``\n\n# Shared\n'
  );
  const parses = cache();
  const warn = vi.fn();
  const [rendered, metadata] = await Promise.all([
    parses.render('/tmp/shared.mg', warn),
    parses.metadata('/tmp/shared.mg', warn),
  ]);
  expect(rendered.metadata).toEqual(metadata.metadata);
  expect(rendered.toc).toEqual(metadata.toc);
  expect(readFile).toHaveBeenCalledTimes(1);
  // Two identical occurrences in the document, not four from two projections.
  expect(warn).toHaveBeenCalledTimes(2);
});

it('re-reads after an ordinary read failure instead of caching the rejection', async () => {
  readFile.mockRejectedValueOnce(new Error('unreadable')).mockResolvedValueOnce('# Recovered\n');
  const parses = cache();
  await expect(parses.metadata('/tmp/retry.mg', vi.fn())).rejects.toThrow('unreadable');
  expect((await parses.metadata('/tmp/retry.mg', vi.fn())).toc[0].title).toBe('Recovered');
  expect(readFile).toHaveBeenCalledTimes(2);
});

it('re-reads after a parse failure', async () => {
  readFile.mockResolvedValueOnce('``embed:bogus:\nx\n``\n').mockResolvedValueOnce('# Recovered\n');
  const parses = cache();
  await expect(parses.render('/tmp/retry.mg', vi.fn())).rejects.toThrow('invalid language');
  expect((await parses.render('/tmp/retry.mg', vi.fn())).toc[0].title).toBe('Recovered');
  expect(readFile).toHaveBeenCalledTimes(2);
});

it('invalidates both projections together while a source read is pending', async () => {
  let resolveStale!: (content: string) => void;
  readFile
    .mockImplementationOnce(
      () =>
        new Promise<string>(resolve => {
          resolveStale = resolve;
        })
    )
    .mockResolvedValueOnce('# Fresh\n');
  const parses = cache();
  const rendered = parses.render('/tmp/shared.mg', vi.fn());
  const metadata = parses.metadata('/tmp/shared.mg', vi.fn());
  parses.invalidate('/tmp/shared.mg');
  resolveStale('# Stale\n');
  for (const result of await Promise.all([rendered, metadata])) {
    expect(result.toc[0].title).toBe('Fresh');
  }
  expect(readFile).toHaveBeenCalledTimes(2);
});

it('retries an in-flight parse invalidated by HMR', async () => {
  let resolveStale!: (content: string) => void;
  readFile
    .mockImplementationOnce(() => new Promise<string>(resolve => (resolveStale = resolve)))
    .mockResolvedValueOnce('# Fresh\n');

  const file = '/tmp/cache-race.mg';
  const plugin = mogPlugin({ mode: 'html', include: ['**/*.mg'] });
  const context = {
    warn: vi.fn(),
    error(message: string): never {
      throw new Error(message);
    },
  };
  const load = plugin.load as (this: typeof context, id: string) => Promise<string | undefined>;
  const staleLoad = load.call(context, file);

  const hotUpdate = plugin.hotUpdate as (ctx: {
    file: string;
    modules: never[];
  }) => Promise<unknown>;
  await hotUpdate({ file, modules: [] });
  resolveStale('# Stale\n');

  const result = await staleLoad;
  expect(result).toContain('Fresh');
  expect(result).not.toContain('Stale');
  expect(readFile).toHaveBeenCalledTimes(2);
});

it('retries an in-flight read failure invalidated by HMR', async () => {
  let rejectStale!: (error: Error) => void;
  readFile
    .mockImplementationOnce(() => new Promise<string>((_, reject) => (rejectStale = reject)))
    .mockResolvedValueOnce('# Fresh\n');

  const file = '/tmp/cache-race.mg';
  const plugin = mogPlugin({ mode: 'html', include: ['**/*.mg'] });
  const context = {
    warn: vi.fn(),
    error(message: string): never {
      throw new Error(message);
    },
  };
  const load = plugin.load as (this: typeof context, id: string) => Promise<string | undefined>;
  const staleLoad = load.call(context, file);

  const hotUpdate = plugin.hotUpdate as (ctx: {
    file: string;
    modules: never[];
  }) => Promise<unknown>;
  await hotUpdate({ file, modules: [] });
  rejectStale(new Error('stale read'));

  const result = await staleLoad;
  expect(result).toContain('Fresh');
  expect(readFile).toHaveBeenCalledTimes(2);
});

it('registers the physical Mog source as a dependency of a generated module', async () => {
  readFile.mockResolvedValue('# Watched\n');

  const file = '/tmp/watched.mg';
  const plugin = mogPlugin({ mode: 'svelte', include: ['**/*.mg'] });
  const addWatchFile = vi.fn();
  const context = {
    addWatchFile,
    warn: vi.fn(),
    error(message: string): never {
      throw new Error(message);
    },
  };
  const load = plugin.load as (this: typeof context, id: string) => Promise<string | undefined>;

  await load.call(context, `\0${file}.svelte`);

  expect(addWatchFile).toHaveBeenCalledWith(file);
});
