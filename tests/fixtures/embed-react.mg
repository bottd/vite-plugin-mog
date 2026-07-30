``meta:
title "Embed React Test"
``

# Testing React Embeds

This is a counter component:

``embed:react:
<button onClick={() => setCount(c => c + 1)}>
  Count: {count}
</button>
``

# Multiple Embeds

First embed:

``embed:react:
<div>Hello from React!</div>
``

Second embed:

``embed:react:
<span>Goodbye from React!</span>
``

# Content After Embeds

This text comes after the embeds.
