<script setup lang="ts">
import 'virtual:mog-arborium.css';
import { computed, ref } from 'vue';
import Index, { metadata as indexMeta } from '../content/index.mg';
import Embeds, { metadata as embedsMeta } from '../content/embeds.mg';

// `pages` is a plain array, so a ref here would reach the template unwrapped.
const pages = [
  { id: 'index', component: Index, meta: indexMeta },
  { id: 'embeds', component: Embeds, meta: embedsMeta },
];

const currentId = ref('index');
const current = computed(() => pages.find(page => page.id === currentId.value) ?? pages[0]);
</script>

<template>
  <nav>
    <span class="logo">vite-plugin-mog</span>
    <button
      v-for="{ id, meta } in pages"
      :key="id"
      :class="{ active: currentId === id }"
      @click="currentId = id"
    >
      {{ meta.title }}
    </button>
  </nav>

  <main>
    <component :is="current.component" />
  </main>
</template>

<style>
body {
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
nav button {
  border: none;
  background: none;
  color: #666;
  font-size: 0.95rem;
  cursor: pointer;
}
nav button.active {
  color: #1a1a1a;
  font-weight: 600;
}
main pre {
  padding: 1rem;
  border-radius: 6px;
  overflow-x: auto;
}
</style>
