// Harness of the source test (enabled=true): next() logs its argument and a re-entrant read of `calls`.
(() => {
  let getCalls;
  const events = [];
  globalThis.observe = function (f) { getCalls = f; };
  globalThis.enabled = function () { return true; };
  globalThis.next = function (previous) { events.push('next:' + previous, 'reenter:' + getCalls()); return previous + 1; };
  process.on('exit', () => { console.log('TRACE:' + events.join(',') + ';calls:' + getCalls()); });
})();
