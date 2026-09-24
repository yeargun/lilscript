// Harness of the source test: the coercion of item() writes values[0] before it is read.
(() => {
  globalThis.values = [1];
  globalThis.item = function () { return { [Symbol.toPrimitive]() { globalThis.values[0] = 2; return 'x'; } }; };
})();
