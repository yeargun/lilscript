// Harness of the source test: first() throws; the test caught it around the program.
(() => {
  const events = [];
  globalThis.first = function () { events.push('first'); throw Error('stop'); };
  globalThis.second = function () { events.push('second'); return 2; };
  process.on('uncaughtException', (error) => { events.push('throw'); });
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
