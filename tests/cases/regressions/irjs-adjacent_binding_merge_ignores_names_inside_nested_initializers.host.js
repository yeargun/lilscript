// Harness of the source test: inspect keeps the value; the test printed 'TRACE:'+result.hydrate after the program.
(() => {
  let result;
  globalThis.inspect = function (value) { result = value; };
  globalThis.consume = function () {};
  process.on('exit', () => { console.log('TRACE:' + result.hydrate); });
})();
