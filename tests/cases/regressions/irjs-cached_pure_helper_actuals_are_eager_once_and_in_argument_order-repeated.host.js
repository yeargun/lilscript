(() => {
  const events = [];
  globalThis.input = function () { events.push('input'); return 3; };
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
