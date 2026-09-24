// Harness of the source test: observe records the escaped object's own keys.
(() => {
  let observed = '';
  globalThis.observe = function (box) { observed = Object.keys(box).join(','); };
  process.on('exit', () => { console.log('TRACE:' + observed); });
})();
