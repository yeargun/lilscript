// Harness of the source test: consume keeps the method; the test printed String(seen.call({})).
(() => {
  let seen = null;
  globalThis.consume = function (value) { seen = value; };
  process.on('exit', () => { console.log(String(seen.call({}))); });
})();
