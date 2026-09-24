// Harness of the source test: the host calls are logged in order.
(() => {
  const events = [];
  globalThis.first = function () { events.push('first'); return 3; };
  globalThis.second = function () { events.push('second'); return 4; };
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
