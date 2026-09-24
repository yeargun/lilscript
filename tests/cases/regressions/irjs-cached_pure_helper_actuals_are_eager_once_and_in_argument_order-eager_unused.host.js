(() => {
  const events = [];
  globalThis.flag = function () { events.push('flag'); return false; };
  globalThis.input = function () { events.push('input'); return 9; };
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
