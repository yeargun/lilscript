import { createSSRApp, defineComponent, h, provide, inject } from "vue";
import { renderToString } from "vue/server-renderer";

const Price = defineComponent({
  props: { value: Number },
  setup(props) {
    const currency = inject("currency");
    return () => h("span", { class: "price" }, `${currency}${props.value.toFixed(2)}`);
  },
});

const App = defineComponent({
  setup() {
    provide("currency", "$");
    const products = [
      { id: 1, name: "Lamp", price: 24 },
      { id: 2, name: "Chair", price: 81 },
      { id: 3, name: "Desk", price: 140 },
    ];
    return () =>
      h("main", { id: "catalog", "data-count": products.length }, [
        h("h1", null, "Catalog"),
        h(
          "ul",
          null,
          products.map((product) =>
            h("li", { key: product.id }, [
              h("span", { class: "name" }, product.name),
              " ",
              h(Price, { value: product.price }),
            ]),
          ),
        ),
      ]);
  },
});

const context = {};
const html = await renderToString(createSSRApp(App), context);
globalThis.__VUELIL_PROJECT_RESULT__ = JSON.stringify({
  html,
  teleports: context.teleports ?? {},
});
