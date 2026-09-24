// Harness of the source test: inspect logs the field value the constructor observes.
(() => {
  const events = [];
  globalThis.inspect = function (value) { events.push('inspect:' + value); };
  globalThis.consume = function () {};
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
