``meta:
title "Mog in React"
description "A Mog document compiled to a React component"
``

# Mog in React

This page is a ``text: .mg`` file. The plugin compiled it to a React component,
and ``text: App.tsx`` imported it like any other.

## Setup

``typescript:
import react from '@vitejs/plugin-react';
import { mogPlugin } from 'vite-plugin-mog';

export default defineConfig({
  plugins: [mogPlugin({ mode: 'react' }), react()],
});
``

Then import the document, with its ``text: meta`` block as a named export:

``jsx:
import Doc, { metadata, toc } from './content/index.mg';

export default () => (
  <>
    <h1>{metadata.title}</h1>
    <Doc />
  </>
);
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
