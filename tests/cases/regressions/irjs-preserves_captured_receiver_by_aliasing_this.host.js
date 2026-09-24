// Harness of the source test: the first consumed value is the method, the next the nested arrow;
// after the program the test called method.call({item:7},9) and then nested().
(() => {
  let method = null, nested = null, calls = 0;
  globalThis.consume = function (value) { if (calls === 0) { method = value; calls = 1; } else nested = value; };
  process.on('exit', () => { console.log([method.call({ item: 7 }, 9), nested()].join(':')); });
})();
