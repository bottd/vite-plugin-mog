import { compile as compileSvelte, parse as parseSvelte } from 'svelte/compiler';
import { compileScript, parse as parseVue } from 'vue/compiler-sfc';
import { injectComponentImports } from '../../src/plugin/components.js';

const components = new Map([['Counter', './Counter']]);

it.each([
  '<script></script>\n<Counter />',
  '<script> \n </script>\n<Counter />',
  '<!-- <script></script> -->\n<script></script>\n<Counter />',
  '<script module>export const title = "T";</script>\n<script></script>\n<Counter />',
  '<script lang="ts"></script>\n<Counter />',
  '<script data-note=">"></script>\n<Counter />',
])('compiles imports injected into an empty Svelte instance script: %s', async source => {
  const output = await injectComponentImports(source, components, 'svelte');
  const compiled = compileSvelte(output, { generate: 'client' });
  expect(compiled.js.code).toContain('import Counter from "./Counter";');
});

it('injects into the Svelte instance script, ignoring comments and module scripts', async () => {
  const source =
    '<!-- <script>example</script> -->\n<script module>export const title = "T";</script>\n<script lang="ts">let count: number = 0;</script>\n<Counter />';
  const output = await injectComponentImports(source, components, 'svelte');
  const ast = parseSvelte(output);
  expect(ast.instance?.content.body[0].type).toBe('ImportDeclaration');
  expect(ast.module?.content.body[0].type).toBe('ExportNamedDeclaration');
  expect(output).toContain('<!-- <script>example</script> -->');
});

it('creates a Svelte instance script when only a commented script exists', async () => {
  const output = await injectComponentImports(
    '<!-- <script>example</script> -->\n<Counter />',
    components,
    'svelte'
  );
  expect(parseSvelte(output).instance?.content.body[0].type).toBe('ImportDeclaration');
});

it('locates Vue setup scripts independent of attribute order and comments', async () => {
  const source =
    '<!-- <script setup>example</script> -->\n<script lang="ts" setup>const count: number = 0;</script>\n<template><Counter /></template>';
  const output = await injectComponentImports(source, components, 'vue');
  const { descriptor, errors } = parseVue(output);
  expect(errors).toEqual([]);
  expect(descriptor.scriptSetup?.content).toContain('import Counter from "./Counter";');
  expect(output).toContain('<!-- <script setup>example</script> -->');
  expect(() => compileScript(descriptor, { id: 'test' })).not.toThrow();
});

it('matches the language of an existing Vue script when adding setup', async () => {
  const source = '<script lang="ts">export default {};</script>\n<template><Counter /></template>';
  const { descriptor } = parseVue(await injectComponentImports(source, components, 'vue'));
  expect(descriptor.scriptSetup?.lang).toBe('ts');
  expect(() => compileScript(descriptor, { id: 'test' })).not.toThrow();
});
