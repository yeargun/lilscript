// Harness of the source test: keep() retains go; after the program the test called go with a
// logging callback and printed the log joined with ','.
(() => {
  let go;
  const seen = [];
  globalThis.keep = function (cb) { go = cb; };
  globalThis.observe = function (v) { seen.push(typeof v); };
  process.on('exit', () => { go(function () { seen.push('called'); }); console.log(seen.join(',')); });
})();
