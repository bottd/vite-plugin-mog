``meta:
title "Mog in Vue"
description "A Mog document compiled to a Vue component"
``

# Mog in Vue

This page is a ``text: .mg`` file. The plugin compiled it to a Vue component,
and ``text: App.vue`` imported it like any other.

## Setup

``typescript:
import vue from '@vitejs/plugin-vue';
import { mogPlugin } from 'vite-plugin-mog';

export default defineConfig({
  plugins: [mogPlugin({ mode: 'vue' }), vue()],
});
``

Then import the document, with its ``text: meta`` block as a named export:

``vue:
<script setup>
import Doc, { metadata, toc } from './content/index.mg';
</script>

<template>
  <h1>{{ metadata.title }}</h1>
  <Doc />
</template>
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
