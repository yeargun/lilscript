import { nodes, release, store } from "./host-state.js";

globalThis.domQueryRoot = (selector) => store(document.querySelector(selector));
globalThis.domCreateElement = (tag) => store(document.createElement(tag));
globalThis.domCreateText = (value) => store(document.createTextNode(value));
globalThis.domAppendChild = (parent, child) => {
  nodes[parent].appendChild(nodes[child]);
};
globalThis.domSetText = (node, value) => {
  nodes[node].data = value;
};
globalThis.domSetAttribute = (node, name, value) => {
  nodes[node].setAttribute(name, value);
};
globalThis.domClear = (node) => {
  const parent = nodes[node];
  for (let child = parent.firstChild; child;) {
    const next = child.nextSibling;
    release(child);
    child = next;
  }
  parent.replaceChildren();
};
globalThis.hostSchedule = (callback) => globalThis.queueMicrotask(callback);
globalThis.registerBenchmarkDispose = (callback) => {
  globalThis.__disposeSolidBenchmark = callback;
};
