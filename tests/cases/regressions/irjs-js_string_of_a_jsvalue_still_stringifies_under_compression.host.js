// Harness of the source test: `consume` records typeof/String(first) and typeof(second);
// `v` is an object whose toString returns 'hello'. The test joined the records with ':'.
(() => {
  globalThis.consume = function (a, b) { console.log([typeof a, String(a), typeof b].join(':')); };
  globalThis.v = { toString() { return 'hello'; } };
})();
