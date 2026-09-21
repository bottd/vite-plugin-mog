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
``attr:
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

A document declares metadata in an `attr` verbatim block written in
[KDL](https://kdl.dev), whose fields become the `metadata` export. Every
`attr` block at the top level merges into the document, in source order, so the
front matter can be split up or appended to further down. A key set twice keeps
its first value, with a build warning. An `attr` block indented under a marker
attaches to that node instead of the document.

A node's `attr` block can render as `data-*` attributes on its element, one per
top-level key in source order: a scalar as written, anything nested as JSON. A
list marker's block lands on its `<li>`, and a link's goes in its name —
`[[target]]((name ``attr: k 1``))` — and lands on the `<a>` or `<img>`. Keys
hold letters, digits, `-` and `_`; they are lowercased, and a repeat keeps its
first value, with a build warning.

This is **off by default**. Data in a document is not necessarily data for the
DOM: a document that carries numbers for a build pipeline should not make every
visitor download them. Turn it on with [`dataAttributes`](#options).

```mog
=hero:
``attr:
impact { all before=0.505 after=0.524 }
``
## Abrams
=
```

```html
<!-- with dataAttributes: true -->
<div class="hero" data-impact='{"all":{"before":0.505,"after":0.524}}'>
  <h2 id="abrams">Abrams</h2>
</div>
```

Read it back with `JSON.parse(element.dataset.impact)`. A value is single-quoted
when that costs less — JSON is full of `"`, and each one would otherwise become
`&quot;` — and double-quoted otherwise. Both parse to the same `dataset` value.

A `data-*` value does not carry its type. `a "true"` and `b #true` both render
as `="true"`, and a string that happens to contain JSON is indistinguishable
from a nested value. Whatever reads the attribute has to know the schema.

### Where an `attr` block attaches

The block belongs to whatever is directly above it, and one blank line changes
what that is:

```mog
=hero:
``attr:
impact 1
``
## Abrams
=
```

Directly under a fence it attaches to the block — `data-impact` on the `<div>`.

```mog
- one
``attr:
impact 1
``
```

Directly under a bullet, with no blank line, it attaches to that list item —
`data-impact` on the `<li>`. The plugin emits a build warning here, because the
next case looks almost identical and means something else.

```mog
- one

``attr:
impact 1
``
```

After a blank line, at the top level, it merges into the document's `metadata`
and renders nothing.

> **Renamed:** this block was `meta`, and node attributes were `data`. Both
> spellings now parse as ordinary verbatim blocks — the content renders as a code
> block and contributes no metadata. The plugin emits a build warning naming the
> file where the old parser would have read one — a `meta` block opening the
> document, a `data` block at its top level — so a stale document is loud rather
> than silent. A code sample that trips it goes quiet once it names its real
> language (`kdl`, `text`).
>
> This tracks an unreleased parser, pinned by commit. A published `mog-parser` is
> on the way; expect further syntax movement until then, and the deprecation
> warning to go away with it.

The [Mog spec](https://github.com/bottd/mog) has the full syntax.

## Options

| Option           | Type                                                   | Description                                                                                                                                                                             |
| ---------------- | ------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mode`           | `'html' \| 'react' \| 'svelte' \| 'vue' \| 'metadata'` | Required. What a `.mg` import compiles to.                                                                                                                                              |
| `theme`          | `string \| { light: string; dark: string }`            | Syntax highlighting theme. A pair switches on `prefers-color-scheme`.                                                                                                                   |
| `include`        | `FilterPattern`                                        | Limit which `.mg` files the plugin handles.                                                                                                                                             |
| `exclude`        | `FilterPattern`                                        | Skip matching `.mg` files.                                                                                                                                                              |
| `componentDir`   | `string`                                               | Directory scanned for components that embeds can use.                                                                                                                                   |
| `components`     | `Record<string, string>`                               | Explicit name to import path map. Takes precedence over `componentDir`.                                                                                                                 |
| `dataAttributes` | `boolean \| string[]`                                  | Which node `attr` keys render as `data-*`. Defaults to `false` — none. `true` renders all, an array selects by top-level key. Root-level blocks are unaffected; they remain `metadata`. |

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

Metadata imports collect the heading outline without rendering HTML or highlighting
code. They report document and metadata warnings; output-specific warnings, such
as dropped link schemes or invalid `data-*` keys, are reported when rendering.
Embed declarations are still validated, so a misspelled embed language is an error.

Build scripts can use the same path directly:

```javascript
import { parseMogMetadata } from 'vite-plugin-mog/parser';

const { metadata, toc, diagnostics } = await parseMogMetadata(source);
```

## Reading documents outside Vite

A build script, a database job or a code generator wants the document, not HTML.
`vite-plugin-mog/parser` exposes the parser on its own — importing it pulls in
neither Vite nor any framework peer, so a script can depend on this package
alone.

```javascript
import { parseMogAst } from 'vite-plugin-mog/parser';

const document = await parseMogAst(await readFile('patch.mg', 'utf8'));
```

It renders nothing: no highlighting or embed extraction. Diagnostics are off by
default. The tree is the parser's own — `{ attributes?, body, diagnostics? }`, each node
`{ kind, attributes?, children?, span?, fence? }` — tagged on `kind` so
TypeScript narrows it without casts. A node's chain (`=hero:abrams:` gives
`hero` and `abrams`) stays in `attributes.entries`, separate from what its
`attr` blocks declared in `attributes.children`.

Pass `{ unfolded: true }` to keep `attr` blocks in the tree as their own nodes
rather than folding them into their owner.

### Document diagnostics

Request `{ diagnostics: true }` to get invalid-KDL, attribute-attachment, and
migration warnings from the same document check that `parseMog` runs:

```javascript
const document = await parseMogAst(source, { diagnostics: true });
for (const message of document.diagnostics) console.warn(message);
```

This checks the already-parsed tree, including when `{ unfolded: true }` is used.
The `diagnostics` key is absent by default and is an array (possibly empty) when
requested. Rendering-specific warnings and plain-value projection warnings are
not included; use `parseMogMetadata` for warnings about repeated metadata keys.

### Plain values

The tree keeps KDL's shape, so `title "Patch"` arrives as a node with one
argument rather than as the string `"Patch"`. Turning that into a plain value
takes a rule — one argument is a scalar, several an array, properties or
children an object, and a key set twice keeps its first value — and it is the
same rule the `metadata` export uses.

Rather than write it again, ask for it:

```javascript
const document = await parseMogAst(source, { plain: true });
document.attributes.plain; // { title: 'Patch' }
```

`plain` appears beside nonempty `children` on document and node attributes, and is
omitted when there is nothing to project. The same Rust code behind `metadata`
computes it, so the two agree by construction — including the `__proto__` guard and
integers wider than JavaScript can hold exactly. It is opt-in because it roughly
doubles the attribute payload. Projection is silent, and repeated keys remain
available in `children`; use `parseMogMetadata` for metadata diagnostics.

### Editing documents in place

Every block-level node carries a `span`, and `attributes.blocks` holds the spans
of the `attr` blocks behind its `children`. Together they are enough to rewrite a
document by splicing text, without a printer and without reformatting anything
the edit did not touch:

- replace `attributes.blocks[0]` to rewrite an existing block;
- insert after a marker's `fence` — its opening line alone — to add one.
  `fence` is there exactly when the marker's `span` covers more than that line,
  so `fence ?? span` is always the marker's own line.

```javascript
const hero = document.body[0];
const [block] = hero.attributes?.blocks ?? [];
const next = block
  ? source.slice(0, block.start) + rewritten + source.slice(block.end)
  : source.slice(0, hero.fence.end) + '\n' + rewritten + source.slice(hero.fence.end);
```

> `span.start` and `span.end` are **UTF-8 byte offsets**, while a JavaScript
> string is indexed in UTF-16 code units. `source.slice(start, end)` is wrong for
> any document containing a non-ASCII character. Splice on `startLine` /
> `endLine`, or slice a `Buffer`.

An inline `attr` block — one written inside a line — contributes no span, because
a span covering the whole line would delete the line. `blocks` being empty means
"nothing to splice", never "no attributes".

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

An embed can sit inside a block or a list item. The plugin builds the
containers around it — the block's `<div>`, the list and its items — as real
elements of the component, so the embed is a true child of them and rules like
`.card > *` reach it the same way in every mode.

The HTML beside an embed is a string, which React and Vue can only mount inside
an element. There it sits in a `display: contents` wrapper: layout is unchanged,
but a selector sees the wrapper, so `.card > p` or `p:first-child` will not match
across it. Svelte needs no wrapper, and an item holding nothing but HTML takes
the string as its own content in every mode. Releases up to 0.2.1 wrapped a
document's HTML in a plain block `<div>`; styles that targeted it as a box need
to move to the content or to your own container.

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
rendering the same two documents, embeds and highlighting included.
`pnpm test:e2e` builds them all to check each output mode.

## Requirements

- The package is ESM-only, including `vite-plugin-mog/parser`; use `import`.
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

`rustc` and `cargo` have to be on `PATH`; outside the dev shell, build with
`nix develop --command pnpm run build`.

### Consuming a local checkout

Use `link:`, not `file:`:

```json
{ "dependencies": { "vite-plugin-mog": "link:../vite-plugin-mog" } }
```

`files` excludes `dist/napi/*.node`, because a published install gets its binary
from a platform package. `file:` copies the package, so it copies the new
JavaScript and no binary — and the loader then falls back to whichever published
platform package is already in the store. The result is new JavaScript calling an
old parser, with nothing to show for it but wrong output.

The two halves therefore check each other: the native binary is stamped with a
digest of the Rust sources and `Cargo.lock` it was built from, the JavaScript
build computes the same digest, and loading the parser throws when they differ.
A version comparison cannot catch this — an unreleased checkout and the published
release it shadows carry the same version string, and released binaries are built
before the version bump.

## License

MIT © [Drake Bott](https://github.com/bottd)
