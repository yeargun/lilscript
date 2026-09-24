(() => {
  const events = [];
  globalThis.first = function () { events.push('first'); return 1; };
  globalThis.ignored = function () { events.push('ignored'); return 2; };
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
