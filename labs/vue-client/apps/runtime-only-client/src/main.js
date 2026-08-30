import {
  Fragment,
  computed,
  createApp,
  createElementBlock,
  createElementVNode,
  nextTick,
  openBlock,
  reactive,
  renderList,
  toDisplayString,
} from "vue";

const _hoistedMain = { class: "inventory" };
const _hoistedList = { id: "inventory-list" };

function render(_ctx) {
  return (
    openBlock(),
    createElementBlock("main", _hoistedMain, [
      createElementVNode("h1", null, toDisplayString(_ctx.title), 1),
      createElementVNode(
        "button",
        { id: "increment", onClick: _ctx.increment },
        `Add (${toDisplayString(_ctx.total)})`,
        9,
        ["onClick"],
      ),
      createElementVNode("ul", _hoistedList, [
        (openBlock(true),
        createElementBlock(
          Fragment,
          null,
          renderList(_ctx.items, (item) =>
            (openBlock(),
            createElementBlock("li", { key: item.id }, toDisplayString(item.label), 1)),
          ),
          128,
        )),
      ]),
    ])
  );
}

const state = reactive({
  title: "Warehouse",
  items: [
    { id: 1, label: "Bolts" },
    { id: 2, label: "Washers" },
  ],
});
const total = computed(() => state.items.length);

const app = createApp({
  setup() {
    const increment = () => {
      const id = state.items.length + 1;
      state.items.push({ id, label: `Part ${id}` });
    };
    return { increment, items: state.items, title: state.title, total };
  },
  render,
});

app.mount("#app");
document.querySelector("#increment").dispatchEvent(new Event("click"));
await nextTick();
globalThis.__VUELIL_PROJECT_RESULT__ = JSON.stringify({
  html: document.querySelector("#app").innerHTML,
  itemCount: document.querySelectorAll("#inventory-list li").length,
});
