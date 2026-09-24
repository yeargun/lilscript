(() => {
  const events = [];
  globalThis.first = function () { events.push('first'); return 1; };
  globalThis.second = function () { events.push('second'); return 2; };
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
