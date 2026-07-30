<script setup lang="ts">
import { ref } from 'vue';

const count = ref(0);
</script>

<template>
  <button @click="count++">Clicked {{ count }} times</button>
</template>

<style scoped>
button {
  padding: 0.4rem 0.9rem;
  border: 1px solid #d0d0d0;
  border-radius: 6px;
  background: white;
  cursor: pointer;
}
</style>
