// Harness of the source test: each operand logs its coercion.
(() => {
  const events = [];
  globalThis.first = function () { return { [Symbol.toPrimitive]() { events.push('left'); return 'L'; } }; };
  globalThis.second = function () { return { [Symbol.toPrimitive]() { events.push('right'); return 'R'; } }; };
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
