"use strict";
const { Fragment, h } = require("../dist/vuelil-packages.cjs").vue_runtime;
function jsx(type, props, key) {
  const { children, ...rest } = props || {};
  if (arguments.length > 2) rest.key = key;
  return h(type, rest, children);
}
exports.Fragment = Fragment;
exports.jsx = jsx;
exports.jsxDEV = jsx;
exports.jsxs = jsx;
