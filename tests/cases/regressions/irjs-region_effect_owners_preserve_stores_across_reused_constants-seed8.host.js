// Harness of the source test: observe() keeps the calls getter; the test printed 'calls:'+getCalls().
(() => {
  let getCalls;
  globalThis.observe = function (f) { getCalls = f; };
  globalThis.read = function () { return 8; };
  process.on('exit', () => { console.log('calls:' + getCalls()); });
})();
