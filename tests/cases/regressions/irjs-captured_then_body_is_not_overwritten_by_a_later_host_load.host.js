// Harness of the source test: report() counts its calls; the test printed the count.
(() => {
  let hits = 0;
  globalThis.report = function () { hits++; };
  process.on('exit', () => { console.log(String(hits)); });
})();
