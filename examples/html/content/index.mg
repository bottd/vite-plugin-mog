``meta:
title "Mog as HTML"
description "A Mog document compiled to an HTML string"
``

# Mog as HTML

This page is a ``text: .mg`` file. In ``text: html`` mode a document compiles to
a plain string plus its ``text: metadata`` and ``text: toc``, with no framework
in sight — ``text: src/index.ts`` drops it into the DOM.

## Setup

``typescript:
import { mogPlugin } from 'vite-plugin-mog';

export default defineConfig({
  plugins: [mogPlugin({ mode: 'html' })],
});
``

``javascript:
import { html, metadata, toc } from './content/index.mg';

document.getElementById('content').innerHTML = html;
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
