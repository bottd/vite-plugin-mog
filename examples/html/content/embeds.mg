``meta:
title "Embeds"
description "Inline markup and document CSS inside a Mog document"
``

# Embeds

The framework modes embed components. ``text: html`` mode has no components to
embed, so an ``text: embed:html`` block passes markup straight through:

``embed:html:
<figure class="mog-note">
  <figcaption>Inline markup, untouched by the parser.</figcaption>
</figure>
``

## Document CSS

An ``text: embed:css`` block styles this document only. Here it is inlined as a
leading ``text: <style>`` tag, so the ``text: html`` export stays self-contained
even when written straight to a file:

``embed:css:
.mog-note {
  padding: 0.75rem 1rem;
  border-left: 3px solid rebeccapurple;
  background: #f3f0f8;
}
``
