import { createApp, nextTick } from "vue";
import App from "./App.vue";

createApp(App).mount("#app");
document.querySelector("#add-task").dispatchEvent(new Event("click"));
await nextTick();
globalThis.__VUELIL_PROJECT_RESULT__ = JSON.stringify({
  html: document.querySelector("#app").innerHTML,
  taskCount: document.querySelectorAll("#task-list li").length,
  summary: document.querySelector("#summary").textContent,
});
