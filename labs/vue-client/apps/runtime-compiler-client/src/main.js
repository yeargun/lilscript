import { computed, createApp, nextTick, ref } from "vue";

const app = createApp({
  setup() {
    const customer = ref("Ada");
    const items = ref([
      { id: 1, label: "Notebook", price: 7 },
      { id: 2, label: "Pencil", price: 2 },
    ]);
    const total = computed(() =>
      items.value.reduce((sum, item) => sum + item.price, 0),
    );
    function addItem() {
      items.value.push({ id: 3, label: "Eraser", price: 3 });
    }
    return {
      addItem,
      customer,
      items,
      total,
    };
  },
  template: `
    <main class="cart">
      <h1>{{ customer }}'s cart</h1>
      <button id="add-item" @click="addItem">Add item</button>
      <ul id="cart-items">
        <li v-for="item in items" :key="item.id">{{ item.label }}: {{ item.price }}</li>
      </ul>
      <strong id="total">Total: {{ total }}</strong>
    </main>
  `,
});

app.mount("#app");
document.querySelector("#add-item").dispatchEvent(new Event("click"));
await nextTick();
globalThis.__VUELIL_PROJECT_RESULT__ = JSON.stringify({
  html: document.querySelector("#app").innerHTML,
  itemCount: document.querySelectorAll("#cart-items li").length,
  total: document.querySelector("#total").textContent,
});
