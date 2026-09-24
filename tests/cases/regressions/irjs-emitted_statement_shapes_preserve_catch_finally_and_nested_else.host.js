// Harness of the source test: flag() pops a fixed script; event() logs; the test printed the log joined with ','.
(() => {
  const events = [];
  const flags = [false, false, true, false, true, true, true, false, false, true];
  globalThis.flag = function () { return flags.shift(); };
  globalThis.event = function (x) { events.push(x); };
  process.on('exit', () => { console.log(events.join(',')); });
})();
