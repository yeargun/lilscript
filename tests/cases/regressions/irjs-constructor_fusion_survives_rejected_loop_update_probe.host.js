// Harness of the source test: consume logs value:items.length of each box.
(() => {
  let seen = '';
  globalThis.consume = function (box) { seen += (seen ? ',' : '') + box.value + ':' + box.items.length; };
  process.on('exit', () => { console.log('TRACE:' + seen); });
})();
