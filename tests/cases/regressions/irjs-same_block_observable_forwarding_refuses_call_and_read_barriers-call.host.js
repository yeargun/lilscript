// Harness of the source test (call barrier): reads, the note() call and the coercion are logged in order.
(() => {
  const events = [];
  const payload = { [Symbol.toPrimitive]() { events.push('coerce'); return 4; } };
  globalThis.input = new Proxy({ first: payload }, { get(object, key) { events.push('get:' + String(key)); return Reflect.get(object, key); } });
  globalThis.note = function () { events.push('note'); };
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
