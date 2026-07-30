<script lang="ts">
  import 'virtual:mog-arborium.css';
  import Index, { metadata as indexMeta } from '../content/index.mg';
  import Embeds, { metadata as embedsMeta } from '../content/embeds.mg';

  const pages = [
    { id: 'index', component: Index, meta: indexMeta },
    { id: 'embeds', component: Embeds, meta: embedsMeta },
  ];

  let currentId = $state('index');
  const current = $derived(pages.find(page => page.id === currentId) ?? pages[0]);
</script>

<nav>
  <span class="logo">vite-plugin-mog</span>
  {#each pages as { id, meta } (id)}
    <button class:active={currentId === id} onclick={() => (currentId = id)}>
      {meta.title}
    </button>
  {/each}
</nav>

<main>
  <current.component />
</main>

<style>
  :global(body) {
    margin: 0 auto;
    max-width: 44rem;
    padding: 0 1.5rem 4rem;
    font-family: system-ui, sans-serif;
    color: #1a1a1a;
    background: #fafafa;
  }
  nav {
    display: flex;
    gap: 1rem;
    align-items: center;
    padding: 1rem 0;
    border-bottom: 1px solid #e5e5e5;
  }
  .logo {
    font-weight: 700;
    margin-right: auto;
  }
  button {
    border: none;
    background: none;
    color: #666;
    font-size: 0.95rem;
    cursor: pointer;
  }
  button.active {
    color: #1a1a1a;
    font-weight: 600;
  }
  main :global(pre) {
    padding: 1rem;
    border-radius: 6px;
    overflow-x: auto;
  }
</style>
