// Harness of the source test: read() returns null once, then 'value'; the test printed 'TRACE:'+reads+':'+fallbacks.
(() => {
  let reads = 0, fallbacks = 0;
  globalThis.read = function () { return reads++ ? 'value' : null; };
  globalThis.fallback = function () { fallbacks++; return 'fallback'; };
  process.on('exit', () => { console.log('TRACE:' + reads + ':' + fallbacks); });
})();
