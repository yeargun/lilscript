// Harness of the source test: consume calls value.factory(null) and tags the result.
(() => {
  let seen = null;
  globalThis.consume = function (value) { seen = value.factory(null); seen.ok = 1; };
  process.on('exit', () => { console.log(String(seen.ok)); });
})();
