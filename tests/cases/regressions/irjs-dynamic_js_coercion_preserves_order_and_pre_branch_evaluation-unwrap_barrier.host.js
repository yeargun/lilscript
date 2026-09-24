// Harness of the source test: the coercion of item() writes the extern global `current`.
(() => {
  globalThis.current = 1;
  globalThis.item = function () { return { [Symbol.toPrimitive]() { globalThis.current = 2; return 'x'; } }; };
  process.on('exit', () => { console.log('TRACE:' + globalThis.current); });
})();
