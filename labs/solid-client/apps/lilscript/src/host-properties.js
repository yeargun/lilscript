import { nodes } from "./host-state.js";

globalThis.domSetBoolAttribute = (node, name, value) => {
  if (value) nodes[node].setAttribute(name, "");
  else nodes[node].removeAttribute(name);
};
globalThis.domSetStringProperty = (node, name, value) => {
  nodes[node][name] = value;
};
globalThis.domSetBoolProperty = (node, name, value) => {
  nodes[node][name] = value;
};
globalThis.domToggleClass = (node, name, value) => {
  for (const token of name.trim().split(/\s+/)) {
    if (token) nodes[node].classList.toggle(token, value);
  }
};
globalThis.domSetStyleProperty = (node, name, value) => {
  if (value === "") nodes[node].style.removeProperty(name);
  else nodes[node].style.setProperty(name, value);
};
