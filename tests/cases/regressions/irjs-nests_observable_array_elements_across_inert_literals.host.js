// Harness of the source test: cb logs its argument; the test printed the log joined with ','.
(() => {
  const events = [];
  globalThis.cb = function (value) { events.push(value); return value; };
  process.on('exit', () => { console.log(events.join(',')); });
})();
