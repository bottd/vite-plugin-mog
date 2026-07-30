# vite-plugin-mog

[![npm version](https://img.shields.io/npm/v/vite-plugin-mog.svg)](https://www.npmjs.com/package/vite-plugin-mog)
[![build status](https://img.shields.io/github/actions/workflow/status/bottd/vite-plugin-mog/release.yml)](https://github.com/bottd/vite-plugin-mog/actions)
[![license](https://img.shields.io/npm/l/vite-plugin-mog.svg)](LICENSE)

Import [Mog](https://github.com/bottd/mog) documents as HTML strings or as React,
Svelte, and Vue components.

## Install

```bash
npm install -D vite-plugin-mog
```

```typescript
// vite.config.ts
import { defineConfig } from 'vite';
import { mogPlugin } from 'vite-plugin-mog';

export default defineConfig({
  plugins: [mogPlugin({ mode: 'svelte' })],
});
```

Add the type reference for your mode to any `.d.ts` in your project so `.mg`
imports are typed:

```typescript
/// <reference types="vite-plugin-mog/svelte" />
```

The other modes are `vite-plugin-mog/react`, `/vue`, `/html`, and `/metadata`.

## A Mog document

```mog
``meta:
title "My Document"
author "Drake Bott"
tags "guide" "intro"
``

# Main Title

Text with **bold**, __italic__, ``text: code``, and ~~strikethrough~~.

## Lists

- Item 1
-- Nested item
- [[https://kdl.dev]]((a link))

.>: Ordered, and in progress
.o: Ordered, and not started

``python:
def greet(name):
    print(f"Hello, {name}!")
``
```

A document declares metadata by opening with a `meta` verbatim block written in
[KDL](https://kdl.dev). Its fields become the `metadata` export. Only the first
block counts; a `meta` block further down is an ordinary verbatim block. The
[Mog spec](https://github.com/bottd/mog) has the full syntax.

## Options

| Option         | Type                                                   | Description                                                             |
| -------------- | ------------------------------------------------------ | ----------------------------------------------------------------------- |
| `mode`         | `'html' \| 'react' \| 'svelte' \| 'vue' \| 'metadata'` | Required. What a `.mg` import compiles to.                              |
| `theme`        | `string \| { light: string; dark: string }`            | Syntax highlighting theme. A pair switches on `prefers-color-scheme`.   |
| `include`      | `FilterPattern`                                        | Limit which `.mg` files the plugin handles.                             |
| `exclude`      | `FilterPattern`                                        | Skip matching `.mg` files.                                              |
| `componentDir` | `string`                                               | Directory scanned for components that embeds can use.                   |
| `components`   | `Record<string, string>`                               | Explicit name to import path map. Takes precedence over `componentDir`. |

## Usage

Every mode exports `metadata` and `toc`. The default export is what changes.

```javascript
// html
import { metadata, html } from './document.mg';
document.body.innerHTML = html;
```

```jsx
// react
import { metadata, Component } from './document.mg';

export default () => <Component />;
```

```svelte
<!-- svelte -->
<script>
  import Document, { metadata } from './document.mg';
</script>

<h1>{metadata.title}</h1>
<Document />
```

```vue
<!-- vue -->
<script setup>
import Document, { metadata } from './document.mg';
</script>

<template>
  <Document />
</template>
```

`toc` holds one entry per heading, with the anchor id the renderer emitted:

```javascript
import { toc } from './document.mg';
// [{ level: 1, title: "Main Title", id: "main-title" }, ...]
```

Append `?metadata` to any import to skip rendering and get metadata only,
whatever the mode:

```javascript
import { metadata, toc } from './document.mg?metadata';
```

## Syntax highlighting

Verbatim blocks carry their language as an attribute (the `python` above).
Highlighting comes from [arborium](https://arborium.bearcove.eu/), which uses
tree-sitter. Set a theme to turn it on:

```typescript
mogPlugin({ mode: 'html', theme: 'GitHub Dark' });
mogPlugin({
  mode: 'html',
  theme: { light: 'GitHub Light', dark: 'Tokyo Night' },
});
```

Themes are named for display, and any spelling that slugs the same works, so
`'github-dark'` finds `'GitHub Dark'`. An unknown name fails at startup and
lists every theme it could have been. See the
[arborium themes](https://github.com/bearcove/arborium?tab=readme-ov-file#themes)
for the full set.

## Embeds

A verbatim block with an `embed` attribute chain drops a component into the
document:

```mog
# Example document
With some regular text

``embed:svelte:
<Chart variant="bar" />
``
```

Point `componentDir` at a directory of components, or map imports yourself:

```typescript
mogPlugin({
  mode: 'svelte',
  componentDir: './src/components',
  components: {
    Chart: './src/lib/Chart.svelte',
  },
});
```

`embed:css` adds document styles, which every framework mode imports as a
regular Vite CSS module:

```mog
``embed:css:
h2 {
  color: red;
}
``
```

In `html` mode the styles are inlined as a leading `<style>` tag instead, so the
`html` export stays self-contained when you write it straight to a file. The
trade-off is that document CSS skips Vite's CSS pipeline there: no PostCSS,
minification, or extraction.

## Examples

`examples/` holds one small project per mode — [svelte](examples/svelte),
[vue](examples/vue), [react](examples/react), [html](examples/html) — each
rendering the same two documents, embeds and highlighting included:

`pnpm test:e2e` builds all examples to test each output mode

## Requirements

- Vite 8+, Node `^20.19` or `>=22.12`
- React 19+, Svelte 5+, or Vue 3+ for the matching mode

Prebuilt native binaries ship for macOS (x64, arm64) and Linux (x64, arm64,
both glibc and musl). Windows is not supported yet.

## Development

This project uses Nix flakes and direnv.

```bash
direnv allow
pnpm install

pnpm test    # JS tests
cargo test   # Rust tests
nix fmt      # lint and format
```

## License

MIT © [Drake Bott](https://github.com/bottd)
