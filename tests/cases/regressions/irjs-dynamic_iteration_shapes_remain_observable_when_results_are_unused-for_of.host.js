// Harness of the source test: a host generator logs when it starts and finishes.
(() => {
  const events = [];
  function* sequence() { events.push('start'); yield 1; events.push('end'); }
  globalThis.input = function () { return sequence(); };
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
