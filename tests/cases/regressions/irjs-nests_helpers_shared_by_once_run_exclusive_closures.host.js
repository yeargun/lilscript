// Harness of the source test: consume calls value.left(1)+value.right(2); the test printed the sum.
(() => {
  let seen = null;
  globalThis.consume = function (value) { seen = value.left(1) + value.right(2); };
  process.on('exit', () => { console.log(String(seen)); });
})();
