<script lang="ts">
  let count = $state(0);
</script>

<button onclick={() => count++}>Clicked {count} times</button>

<style>
  button {
    padding: 0.4rem 0.9rem;
    border: 1px solid #d0d0d0;
    border-radius: 6px;
    background: white;
    cursor: pointer;
  }
</style>
