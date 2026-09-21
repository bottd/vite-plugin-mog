import {
  compile as compileSvelte,
  parse as parseSvelte,
  preprocess as preprocessSvelte,
} from 'svelte/compiler';
import { compileScript, parse as parseVue } from 'vue/compiler-sfc';
import { injectComponentImports } from '../../src/plugin/components.js';

const components = new Map([['Counter', './Counter']]);

it.each(['script', 'style'])('preserves literal <%s> strings in Svelte expressions', async tag => {
  const literal = `<p>{"<${tag}>"}</p>`;
  const source = `${literal}\n<script>let count = 0;</script>\n<Counter />\n<style>p { color: red; }</style>`;
  expect(() => compileSvelte(source, { generate: 'client' })).not.toThrow();
  const output = await injectComponentImports(source, components, 'svelte');
  expect(output).toContain(literal);
  expect(parseSvelte(output).instance?.content.body[0].type).toBe('ImportDeclaration');
  expect(compileSvelte(output, { generate: 'client' }).js.code).toContain(
    'import Counter from "./Counter";'
  );
});

it.each([
  '<script></script>\n<Counter />',
  '<script> \n </script>\n<Counter />',
  '<!-- <script></script> -->\n<script></script>\n<Counter />',
  '<script module>export const title = "T";</script>\n<script></script>\n<Counter />',
  '<script lang="ts"></script>\n<Counter />',
  '<script data-note=">"></script>\n<Counter />',
  '<div>{"<div>"}</div>\n<script></script>\n<Counter />',
  '<script module>export const title = "😀";</script>\r\n<script></script>\r\n<Counter />',
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

it.each([
  '',
  '<script></script>',
  '<script>let count = 0;</script>',
  '<script module>export const title = "T";</script>',
])('preserves unprocessed styles when injecting Svelte imports: %s', async script => {
  const scss = '$color: red;\n$example: "<script></script>";\np { color: $color; }';
  const styleTag = `<style lang="scss">${scss}</style>`;
  const source = `<Counter />\n<p>Styled</p>\n${styleTag}\n${script}`;
  const output = await injectComponentImports(source, components, 'svelte');
  expect(output).toContain(styleTag);

  const style = vi.fn(({ content, attributes }) => {
    expect(attributes.lang).toBe('scss');
    expect(content).toBe(scss);
    return { code: 'p { color: red; }' };
  });
  const processed = await preprocessSvelte(output, { style });
  const compiled = compileSvelte(processed.code, { generate: 'client' });
  expect(style).toHaveBeenCalledOnce();
  expect(compiled.js.code).toContain('import Counter from "./Counter";');
  expect(compiled.css?.code).toContain('color: red');
});

it('allows template preprocessing after Svelte imports are injected', async () => {
  const source = '{% if visible %}<Counter />{% endif %}\n<script></script>';
  const output = await injectComponentImports(source, components, 'svelte');
  expect(output).toContain('{% if visible %}<Counter />{% endif %}');
  const processed = await preprocessSvelte(output, {
    markup: ({ content }) => ({
      code: content.replace('{% if visible %}', '{#if true}').replace('{% endif %}', '{/if}'),
    }),
  });
  expect(() => compileSvelte(processed.code, { generate: 'client' })).not.toThrow();
});

it('allows script preprocessing after Svelte imports are injected', async () => {
  const output = await injectComponentImports(
    '<script>const count = @count;</script>\n<Counter />{count}',
    components,
    'svelte'
  );
  expect(output).toContain('const count = @count;');
  const processed = await preprocessSvelte(output, {
    script: ({ content }) => ({ code: content.replace('@count', '42') }),
  });
  expect(() => compileSvelte(processed.code, { generate: 'client' })).not.toThrow();
});

it('keeps scripts inside svelte:head separate from the instance script', async () => {
  const head = '<svelte:head><script src="/analytics.js"></script></svelte:head>';
  const output = await injectComponentImports(`${head}\n<Counter />`, components, 'svelte');
  expect(output).toContain(head);
  expect(parseSvelte(output).instance?.content.body[0].type).toBe('ImportDeclaration');
  expect(() => compileSvelte(output, { generate: 'client' })).not.toThrow();
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
