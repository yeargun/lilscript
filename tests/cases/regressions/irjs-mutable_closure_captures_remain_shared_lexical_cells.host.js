// Harness of the source test: keep() stores the callbacks; the test called both after the program.
(() => {
  const callbacks = [];
  globalThis.keep = function (callback) { callbacks.push(callback); };
  process.on('exit', () => { console.log('TRACE:' + callbacks[0]() + ':' + callbacks[1]()); });
})();
