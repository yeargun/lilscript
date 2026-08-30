<script setup>
import { computed, ref } from "vue";
import TaskRow from "./TaskRow.vue";

const tasks = ref([
  { id: 1, title: "Audit bundle", done: true },
  { id: 2, title: "Ship report", done: false },
]);
const completed = computed(() => tasks.value.filter((task) => task.done).length);

function addTask() {
  tasks.value.push({ id: 3, title: "Publish evidence", done: false });
}
</script>

<template>
  <main class="board">
    <header>
      <h1>Release board</h1>
      <p id="summary">{{ completed }} of {{ tasks.length }} complete</p>
    </header>
    <button id="add-task" type="button" @click="addTask">Add task</button>
    <ul id="task-list">
      <TaskRow v-for="task in tasks" :key="task.id" :task="task" />
    </ul>
  </main>
</template>

<style scoped>
.board { max-width: 42rem; margin: 2rem auto; color: #17202a; }
header { display: flex; align-items: baseline; justify-content: space-between; }
button { border: 0; padding: 0.6rem 1rem; background: #235347; color: white; }
</style>
