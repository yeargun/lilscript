// Harness of the source test: records the name of the wrapper stored in an aggregate field.
(() => {
  let trace = '';
  globalThis.consume = function (value) { trace = value.handle.name; };
  process.on('exit', () => { console.log('TRACE:' + trace); });
})();
