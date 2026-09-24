// Harness of the source test: mutate() overwrites the struct's field.
(() => {
  globalThis.mutate = function (stats) { stats.total = 99; return 1; };
})();
