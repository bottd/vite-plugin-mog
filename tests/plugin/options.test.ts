import { mogPlugin } from '../../src/plugin/index.js';
import type { GeneratorMode } from '../../src/plugin/generators';
import { loadCode } from './fixtures.js';

const MODES: GeneratorMode[] = ['html', 'svelte', 'react', 'vue', 'metadata'];

it.each(MODES)('accepts the %s mode', mode => {
  expect(mogPlugin({ mode }).name).toBe('vite-plugin-mog');
});

it('rejects an unknown mode instead of emitting an undefined module', () => {
  // Without the guard the plugin loads and silently produces nothing.
  expect(() => mogPlugin({ mode: 'nope' as GeneratorMode })).toThrow(
    /Invalid mode "nope".*Expected one of: html, svelte, vue, react, metadata/s
  );
});

it('rejects inherited object properties as modes', () => {
  expect(() => mogPlugin({ mode: 'toString' as GeneratorMode })).toThrow(/Invalid mode/);
});

it('rejects unknown Arborium themes and lists the real ones', () => {
  expect(() => mogPlugin({ mode: 'html', theme: 'definitely-not-a-theme' })).toThrow(
    /Unknown Arborium theme .*Available themes: .*GitHub Dark/s
  );
});

it.each(['GitHub Dark', 'github-dark'])('accepts the %s theme spelling', theme => {
  // The README documented a slug long before the lookup accepted one.
  expect(() => mogPlugin({ mode: 'html', theme })).not.toThrow();
});

it('emits a theme pair behind prefers-color-scheme', async () => {
  const plugin = mogPlugin({ mode: 'html', theme: { light: 'GitHub Light', dark: 'GitHub Dark' } });
  const css = await loadCode(plugin, '\0virtual:mog-arborium.css');

  expect(css).toContain('@media (prefers-color-scheme: light)');
  expect(css).toContain('@media (prefers-color-scheme: dark)');
});
