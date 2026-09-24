// Harness of the source test: the loose compare's coercion throws; the test caught it around the program.
(() => {
  const events = [];
  globalThis.item = function () { return { [Symbol.toPrimitive]() { events.push('coerce'); throw Error('stop'); } }; };
  globalThis.flag = function () { return false; };
  process.on('uncaughtException', (error) => { events.push('throw'); });
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
