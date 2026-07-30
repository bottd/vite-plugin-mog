``meta:
title "Embed Test"
``

# Testing Svelte Embeds

This is a counter component:

``embed:svelte:
<script>
  let count = 0;
</script>

<button on:click={() => count++}>
  Count: {count}
</button>

<style>
  button {
    background: blue;
    color: white;
  }
</style>
``

# Multiple Embeds

First embed:

``embed:svelte:
<div>Hello from Svelte!</div>
``

Second embed:

``embed:svelte:
<script>
  let name = "World";
</script>

<h1>Hello {name}!</h1>
``

# Content Between Embeds

This text is between two embed blocks.

``embed:svelte:
<p>Another component</p>
``

This text comes after the third embed.
