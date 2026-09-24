// Harness of the source test: host calls are logged in order.
(() => {
  const events = [];
  globalThis.read = function () { events.push('read'); return 9; };
  globalThis.observe = function (box) { events.push('observe:' + box.value); };
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
