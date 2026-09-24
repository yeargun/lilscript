// Harness of the source test: keep() calls the callback at once and records its result.
(() => {
  let seen = 0;
  globalThis.read = function () { return 7; };
  globalThis.keep = function (callback) { seen = callback(); };
  process.on('exit', () => { console.log('TRACE:' + seen); });
})();
