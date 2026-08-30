import { Fragment, h } from "../vue.runtime.js";
function jsx(type, props, key) {
  const { children, ...rest } = props ?? {};
  if (arguments.length > 2) rest.key = key;
  return h(type, rest, children);
}
export { Fragment, jsx, jsx as jsxDEV, jsx as jsxs };
