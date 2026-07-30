``meta:
title "Mog in Svelte"
description "A Mog document compiled to a Svelte component"
``

# Mog in Svelte

This page is a ``text: .mg`` file. The plugin compiled it to a Svelte component,
and ``text: App.svelte`` imported it like any other.

## Setup

``typescript:
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { mogPlugin } from 'vite-plugin-mog';

export default defineConfig({
  plugins: [mogPlugin({ mode: 'svelte' }), svelte()],
});
``

Then import the document, with its ``text: meta`` block as a named export:

``svelte:
<script>
  import Doc, { metadata, toc } from './content/index.mg';
</script>

<h1>{metadata.title}</h1>
<Doc />
``

## Syntax

Text with **bold**, __italic__, ``inline code``, and ~~strikethrough~~.

- A list item
-- A nested one
- [[https://github.com/bottd/mog]]((A link))

.>: An in-progress task
.o: One not started yet

> Quoted text, for when someone said it better.

Verbatim blocks carry their language, and arborium highlights them:

``rust:
fn main() {
    println!("Hello from a highlighted block");
}
``
