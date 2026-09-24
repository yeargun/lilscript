(() => {
  const events = [];
  globalThis.input = function () { events.push('input'); return -2; };
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
