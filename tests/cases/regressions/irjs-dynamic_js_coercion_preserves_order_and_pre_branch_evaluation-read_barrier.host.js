// Harness of the source test: the coercion of item() writes the extern global `state`.
(() => {
  globalThis.state = 1;
  globalThis.item = function () { return { [Symbol.toPrimitive]() { globalThis.state = 2; return 'x'; } }; };
  process.on('exit', () => { console.log('TRACE:' + globalThis.state); });
})();
