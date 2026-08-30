<script setup>
defineProps({
  task: { type: Object, required: true },
});
</script>

<template>
  <li :class="{ complete: task.done }" :data-task-id="task.id">
    <strong>{{ task.title }}</strong>
    <span>{{ task.done ? "done" : "open" }}</span>
  </li>
</template>

<style scoped>
li { display: flex; justify-content: space-between; padding: 0.5rem 0; }
.complete { opacity: 0.65; text-decoration: line-through; }
</style>
