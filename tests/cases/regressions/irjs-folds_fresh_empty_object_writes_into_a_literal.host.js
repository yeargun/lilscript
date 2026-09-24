// Harness of the source test: consume keeps the object; the test printed JSON.stringify(seen).
(() => {
  let seen = null;
  globalThis.consume = function (value) { seen = value; };
  process.on('exit', () => { console.log(JSON.stringify(seen)); });
})();
