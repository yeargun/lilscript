// Harness of the source test: keep() calls the kept walk at once and throws if `last` was shadowed.
(() => {
  globalThis.keep = function (cb) { if (cb(["hello", "world"], "src") !== "world") throw new Error('shadowed last'); };
})();
