// Harness of the source test: effect() throws; the test caught it around the program.
(() => {
  const events = [];
  globalThis.effect = function () { events.push('effect'); throw Error('stop'); };
  globalThis.flag = function () { events.push('flag'); return false; };
  process.on('uncaughtException', (error) => { events.push('throw'); });
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
