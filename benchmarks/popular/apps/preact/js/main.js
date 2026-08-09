import { h, render, Component, Fragment, options } from "preact";
import {
  useState,
  useLayoutEffect,
  useRef,
  useMemo,
  useCallback,
} from "preact/hooks";
import { createDocument } from "../shared/dom.js";
import { serializeElement } from "../shared/canon.js";

options.debounceRendering = (fn) => fn();
options.requestAnimationFrame = (cb) => cb();

const document = createDocument();
globalThis.document = document;

const api = {};

class Badge extends Component {
  render({ n }) {
    return h("div", { class: "badge", "data-n": String(n) }, String(n));
  }
}

function App() {
  const [count, setCount] = useState(0);
  const [items, setItems] = useState(["a", "b"]);
  const countRef = useRef(null);
  const doubled = useMemo(() => count * 2, [count]);
  const inc = useCallback(() => setCount((value) => value + 1), []);

  useLayoutEffect(() => {
    if (countRef.current) {
      countRef.current.setAttribute("data-doubled", String(doubled));
    }
  }, [doubled]);

  api.inc = inc;
  api.add = (item) => setItems((list) => list.concat([item]));

  return h(
    Fragment,
    null,
    h(
      "div",
      { class: "app", id: "root" },
      h("span", { id: "count", ref: countRef }, String(count)),
      h("span", { id: "memo" }, String(doubled)),
      h(
        "ul",
        null,
        items.map((item) => h("li", { key: item }, item)),
      ),
      h(Badge, { n: count, key: "badge" }),
    ),
  );
}

const mount = document.createElement("div");
render(h(App), mount);
const before = serializeElement(mount.firstChild);
api.inc();
api.add("c");
const after = serializeElement(mount.firstChild);
console.log(`preact:${before}|${after}`);
