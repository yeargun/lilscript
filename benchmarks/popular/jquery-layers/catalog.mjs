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

const dataQueueFiles = [
  ...coreKernelFiles,
  { file: "var/rnothtmlwhite.js", binding: "rnothtmlwhite" },
  { file: "callbacks.js", binding: "jQuery" },
  { file: "deferred.js", binding: "jQuery" },
  { file: "deferred/exceptionHook.js", binding: "jQuery" },
  { file: "core/camelCase.js", binding: "camelCase" },
  { file: "core/access.js", binding: "access" },
  { file: "data/var/acceptData.js", binding: "acceptData" },
  { file: "data/Data.js", binding: "Data" },
  { file: "data/var/dataPriv.js", binding: "dataPriv" },
  { file: "data/var/dataUser.js", binding: "dataUser" },
  { file: "data.js", binding: "jQuery" },
  { file: "queue.js", binding: "jQuery" },
];

const selectorFiles = [
  ...dataQueueFiles,
  { file: "core/nodeName.js", binding: "nodeName" },
  { file: "var/pop.js", binding: "pop" },
  { file: "var/sort.js", binding: "sort" },
  { file: "var/splice.js", binding: "splice" },
  { file: "var/whitespace.js", binding: "whitespace" },
  { file: "var/rtrimCSS.js", binding: "rtrimCSS" },
  { file: "selector/contains.js", binding: "jQuery" },
  { file: "selector/escapeSelector.js", binding: "jQuery" },
  { file: "selector.js", binding: "jQuery" },
];

const domCoreFiles = [
  ...selectorFiles,
  { file: "core/var/rsingleTag.js", binding: "rsingleTag" },
  { file: "traversing/var/rneedsContext.js", binding: "rneedsContext" },
  { file: "traversing/findFilter.js", binding: "jQuery" },
  { file: "core/init.js", binding: "init" },
  { file: "traversing/var/dir.js", binding: "dir" },
  { file: "traversing/var/siblings.js", binding: "siblings" },
  { file: "traversing.js", binding: "jQuery" },
  { file: "core/readyException.js", binding: "jQuery" },
  { file: "core/ready.js", binding: "jQuery" },
];

const attributesFiles = [
  ...domCoreFiles,
  { file: "core/stripAndCollapse.js", binding: "stripAndCollapse" },
  { file: "attributes/support.js", binding: "support" },
  { file: "attributes/attr.js", binding: "jQuery" },
  { file: "attributes/prop.js", binding: "jQuery" },
  { file: "attributes/classes.js", binding: "jQuery" },
  { file: "attributes/val.js", binding: "jQuery" },
  { file: "attributes.js", binding: "jQuery" },
];

const eventFiles = [
  ...attributesFiles,
  { file: "var/documentElement.js", binding: "documentElement" },
  { file: "var/rcheckableType.js", binding: "rcheckableType" },
  { file: "event.js", binding: "jQuery" },
  { file: "event/trigger.js", binding: "jQuery" },
];

const manipulationFiles = [
  ...eventFiles,
  { file: "core/isAttached.js", binding: "isAttached" },
  { file: "core/support.js", binding: "support" },
  { file: "manipulation/var/rtagName.js", binding: "rtagName" },
  { file: "manipulation/var/rscriptType.js", binding: "rscriptType" },
  { file: "manipulation/support.js", binding: "support" },
  { file: "manipulation/wrapMap.js", binding: "wrapMap" },
  { file: "manipulation/getAll.js", binding: "getAll" },
  { file: "manipulation/setGlobalEval.js", binding: "setGlobalEval" },
  { file: "manipulation/buildFragment.js", binding: "buildFragment" },
  { file: "core/parseHTML.js", binding: "jQuery" },
  { file: "manipulation.js", binding: "jQuery" },
  { file: "wrap.js", binding: "jQuery" },
];

const cssFiles = [
  ...manipulationFiles,
  { file: "var/pnum.js", binding: "pnum" },
  { file: "var/rcssNum.js", binding: "rcssNum" },
  { file: "css/var/rnumnonpx.js", binding: "rnumnonpx" },
  { file: "css/var/rcustomProp.js", binding: "rcustomProp" },
  { file: "css/var/cssExpand.js", binding: "cssExpand" },
  { file: "css/var/getStyles.js", binding: "getStyles" },
  { file: "css/var/swap.js", binding: "swap" },
  { file: "css/var/rboxStyle.js", binding: "rboxStyle" },
  { file: "css/var/isHiddenWithinTree.js", binding: "isHiddenWithinTree" },
  { file: "css/curCSS.js", binding: "curCSS" },
  { file: "css/adjustCSS.js", binding: "adjustCSS" },
  { file: "css/addGetHookIf.js", binding: "addGetHookIf" },
  { file: "css/support.js", binding: "support" },
  { file: "css/finalPropName.js", binding: "finalPropName" },
  { file: "css/showHide.js", binding: "showHide" },
  { file: "css.js", binding: "jQuery" },
  { file: "css/hiddenVisibleSelectors.js", binding: "jQuery" },
];

const ajaxFiles = [
  ...cssFiles,
  { file: "core/parseXML.js", binding: "jQuery" },
  { file: "serialize.js", binding: "jQuery" },
  { file: "ajax/var/location.js", binding: "location" },
  { file: "ajax/var/nonce.js", binding: "nonce" },
  { file: "ajax/var/rquery.js", binding: "rquery" },
  { file: "ajax.js", binding: "jQuery" },
  { file: "ajax/xhr.js", binding: "jQuery" },
  { file: "ajax/script.js", binding: "jQuery" },
  { file: "ajax/jsonp.js", binding: "jQuery" },
  { file: "ajax/load.js", binding: "jQuery" },
];

const effectsFiles = [
  ...ajaxFiles,
  { file: "effects/Tween.js", binding: "Tween" },
  { file: "effects.js", binding: "jQuery" },
  { file: "effects/animatedSelector.js", binding: "jQuery" },
];

const fullFiles = [
  ...effectsFiles,
  { file: "queue/delay.js", binding: "jQuery" },
  { file: "manipulation/_evalUrl.js", binding: "jQuery" },
  { file: "offset.js", binding: "jQuery" },
  { file: "dimensions.js", binding: "jQuery" },
  { file: "deprecated/ajax-event-alias.js", binding: "jQuery" },
  { file: "deprecated/event.js", binding: "jQuery" },
  { file: "deprecated.js", binding: "jQuery" },
  { file: "exports/amd.js", binding: "jQuery" },
  { file: "exports/global.js", binding: "jQuery" },
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
  {
    id: "data-queue",
    title: "jQuery.data, queue, and dequeue on top of deferred",
    dependsOn: ["deferred"],
    exports: ["jQuery"],
    lilEntry: "ports/jquery/layers/data-queue.lil",
    verify: "jquery-layers/layers/data-queue/verify.mjs",
    upstreamFiles: dataQueueFiles,
  },
  {
    id: "selector",
    title: "Sizzle find, expr, uniqueSort, contains, and escapeSelector",
    dependsOn: ["data-queue"],
    exports: ["jQuery"],
    lilEntry: "ports/jquery/layers/selector.lil",
    verify: "jquery-layers/layers/selector/verify.mjs",
    upstreamFiles: selectorFiles,
  },
  {
    id: "dom-core",
    title: "init, traversing, and ready on top of selector",
    dependsOn: ["selector"],
    exports: ["jQuery"],
    lilEntry: "ports/jquery/layers/dom-core.lil",
    verify: "jquery-layers/layers/dom-core/verify.mjs",
    upstreamFiles: domCoreFiles,
  },
  {
    id: "attributes",
    title: "attr, prop, classes, and val on top of dom-core",
    dependsOn: ["dom-core"],
    exports: ["jQuery"],
    lilEntry: "ports/jquery/layers/attributes.lil",
    verify: "jquery-layers/layers/attributes/verify.mjs",
    upstreamFiles: attributesFiles,
  },
  {
    id: "event",
    title: "event on/off/trigger on top of attributes",
    dependsOn: ["attributes"],
    exports: ["jQuery"],
    lilEntry: "ports/jquery/layers/event.lil",
    verify: "jquery-layers/layers/event/verify.mjs",
    upstreamFiles: eventFiles,
  },
  {
    id: "manipulation",
    title: "dom manipulation, parseHTML, and wrap",
    dependsOn: ["event"],
    exports: ["jQuery"],
    lilEntry: "ports/jquery/layers/manipulation.lil",
    verify: "jquery-layers/layers/manipulation/verify.mjs",
    upstreamFiles: manipulationFiles,
  },
  {
    id: "css",
    title: "css, show/hide, and hidden/visible selectors",
    dependsOn: ["manipulation"],
    exports: ["jQuery"],
    lilEntry: "ports/jquery/layers/css.lil",
    verify: "jquery-layers/layers/css/verify.mjs",
    upstreamFiles: cssFiles,
  },
  {
    id: "ajax",
    title: "ajax, serialize, and transports",
    dependsOn: ["css"],
    exports: ["jQuery"],
    lilEntry: "ports/jquery/layers/ajax.lil",
    verify: "jquery-layers/layers/ajax/verify.mjs",
    upstreamFiles: ajaxFiles,
  },
  {
    id: "effects",
    title: "animate, Tween, and :animated",
    dependsOn: ["ajax"],
    exports: ["jQuery"],
    lilEntry: "ports/jquery/layers/effects.lil",
    verify: "jquery-layers/layers/effects/verify.mjs",
    upstreamFiles: effectsFiles,
  },
  {
    id: "full",
    title: "complete jQuery entry versus official jquery.min.js",
    dependsOn: ["effects"],
    exports: ["jQuery"],
    lilEntry: "ports/jquery/entry.lil",
    verify: "jquery-layers/layers/full/verify.mjs",
    officialMin: "node_modules/jquery/dist/jquery.min.js",
    upstreamFiles: fullFiles,
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
