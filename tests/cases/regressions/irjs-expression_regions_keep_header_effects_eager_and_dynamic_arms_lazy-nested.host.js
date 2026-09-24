// Harness of the source test: effect() throws; the test caught it around the program.
(() => {
  const events = [];
  globalThis.outer = function () { events.push('outer'); return true; };
  globalThis.effect = function () { events.push('effect'); throw Error('stop'); };
  globalThis.inner = function () { events.push('inner'); return false; };
  process.on('uncaughtException', (error) => { events.push('throw'); });
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
