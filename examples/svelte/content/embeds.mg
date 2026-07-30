``meta:
title "Embeds"
description "Live components and document CSS inside a Mog document"
``

# Embeds

An ``text: embed:svelte`` block drops a real component into the document. This
one is ``text: src/components/Counter.svelte``, found by ``text: componentDir``:

``embed:svelte:
<Counter />
``

The component above is live — click it. Everything around it is still Mog.

## Document CSS

An ``text: embed:css`` block styles this document only. The framework modes
hand it to Vite as a regular CSS module:

``embed:css:
.mog-note {
  padding: 0.75rem 1rem;
  border-left: 3px solid rebeccapurple;
  background: #f3f0f8;
}
``

``embed:svelte:
<p class="mog-note">Styled by this document's own CSS.</p>
``
