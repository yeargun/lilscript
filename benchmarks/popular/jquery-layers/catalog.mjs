export const jqueryVersion = "3.7.1";

export const planned = [
  "utilities",
  "core-kernel",
  "callbacks",
  "deferred",
  "data-queue",
  "selector",
  "dom-core",
  "attributes",
  "event",
  "manipulation",
  "css",
  "ajax",
  "effects",
  "full",
];

const coreKernelFiles = [
  { file: "var/arr.js", binding: "arr" },
  { file: "var/getProto.js", binding: "getProto" },
  { file: "var/slice.js", binding: "slice" },
  { file: "var/flat.js", binding: "flat" },
  { file: "var/push.js", binding: "push" },
  { file: "var/indexOf.js", binding: "indexOf" },
  { file: "var/class2type.js", binding: "class2type" },
  { file: "var/toString.js", binding: "toString" },
  { file: "var/hasOwn.js", binding: "hasOwn" },
  { file: "var/fnToString.js", binding: "fnToString" },
  { file: "var/ObjectFunctionString.js", binding: "ObjectFunctionString" },
  { file: "var/support.js", binding: "support" },
  { file: "var/isFunction.js", binding: "isFunction" },
  { file: "var/isWindow.js", binding: "isWindow" },
  { file: "var/document.js", binding: "document" },
  { file: "core/DOMEval.js", binding: "DOMEval" },
  { file: "core/toType.js", binding: "toType" },
  { file: "core.js", binding: "jQuery" },
];

export const layers = [
  {
    id: "utilities",
    title: "Leaf type and string helpers",
    dependsOn: [],
    exports: [
      "isFunction",
      "isWindow",
      "toType",
      "camelCase",
      "nodeName",
      "stripAndCollapse",
    ],
    lilEntry: "ports/jquery/layers/utilities.lil",
    verify: "jquery-layers/layers/utilities/verify.mjs",
    upstreamFiles: [
      { file: "var/class2type.js", binding: "class2type" },
      { file: "var/toString.js", binding: "toString" },
      { file: "var/isFunction.js", binding: "isFunction" },
      { file: "var/isWindow.js", binding: "isWindow" },
      { file: "var/rnothtmlwhite.js", binding: "rnothtmlwhite" },
      { file: "core/toType.js", binding: "toType" },
      { file: "core/camelCase.js", binding: "camelCase" },
      { file: "core/nodeName.js", binding: "nodeName" },
      { file: "core/stripAndCollapse.js", binding: "stripAndCollapse" },
    ],
    afterBindings: [
      {
        after: "class2type",
        source: "core.js",
        note: "class2type populate without jQuery.each",
        code: `"Boolean Number String Function Array Date RegExp Object Error Symbol".split(" ").forEach(function (name) {
	class2type["[object " + name + "]"] = name.toLowerCase();
});`,
      },
    ],
  },
  {
    id: "core-kernel",
    title: "Constructor, fn core, extend, and collection helpers",
    dependsOn: ["utilities"],
    exports: ["jQuery"],
    lilEntry: "ports/jquery/layers/core-kernel.lil",
    verify: "jquery-layers/layers/core-kernel/verify.mjs",
    upstreamFiles: coreKernelFiles,
  },
  {
    id: "callbacks",
    title: "jQuery.Callbacks on top of core-kernel",
    dependsOn: ["core-kernel"],
    exports: ["jQuery"],
    lilEntry: "ports/jquery/layers/callbacks.lil",
    verify: "jquery-layers/layers/callbacks/verify.mjs",
    upstreamFiles: [
      ...coreKernelFiles,
      { file: "var/rnothtmlwhite.js", binding: "rnothtmlwhite" },
      { file: "callbacks.js", binding: "jQuery" },
    ],
  },
  {
    id: "deferred",
    title: "jQuery.Deferred and when on top of callbacks",
    dependsOn: ["callbacks"],
    exports: ["jQuery"],
    lilEntry: "ports/jquery/layers/deferred.lil",
    verify: "jquery-layers/layers/deferred/verify.mjs",
    upstreamFiles: [
      ...coreKernelFiles,
      { file: "var/rnothtmlwhite.js", binding: "rnothtmlwhite" },
      { file: "callbacks.js", binding: "jQuery" },
      { file: "deferred.js", binding: "jQuery" },
      { file: "deferred/exceptionHook.js", binding: "jQuery" },
    ],
  },
];

export function layerById(id) {
  const layer = layers.find((entry) => entry.id === id);
  if (!layer) {
    throw new Error(
      `unknown jquery layer ${JSON.stringify(id)}; implemented: ${layers
        .map((entry) => entry.id)
        .join(", ")}; planned: ${planned.join(" → ")}`,
    );
  }
  return layer;
}
