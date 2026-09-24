// Harness of the source test: read() is null once, then [1,2]; every host call is logged.
(() => {
  let reads = 0;
  const events = [];
  globalThis.read = function () { events.push('read'); return reads++ ? [1, 2] : null; };
  globalThis.fallback = function () { events.push('fallback'); return 7; };
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
