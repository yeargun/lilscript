// Harness of the source test: read() is null once, then [9]; every host call is logged.
(() => {
  let reads = 0;
  const events = [];
  globalThis.read = function () { events.push('read'); return reads++ ? [9] : null; };
  globalThis.index = function () { events.push('index'); return 0; };
  globalThis.fallback = function () { events.push('fallback'); return 7; };
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
