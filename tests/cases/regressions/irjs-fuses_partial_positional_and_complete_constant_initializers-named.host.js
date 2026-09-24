// Harness of the source test: observe records the escaped object's own keys.
(() => {
  let observed = '';
  globalThis.observe = function (flags) { observed = Object.keys(flags).join(','); };
  process.on('exit', () => { console.log('TRACE:' + observed); });
})();
