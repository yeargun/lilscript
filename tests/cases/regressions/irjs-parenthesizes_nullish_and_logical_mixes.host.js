// Harness of the source test: first() is null once, then 'first'; second() is always null; calls are logged.
(() => {
  let calls = 0;
  const events = [];
  globalThis.first = function () { events.push('first'); return calls++ ? 'first' : null; };
  globalThis.second = function () { events.push('second'); return null; };
  globalThis.third = function () { events.push('third'); return 'third'; };
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
