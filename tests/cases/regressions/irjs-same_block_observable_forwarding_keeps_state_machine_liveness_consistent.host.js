// Harness of the source test: a proxy logs reads; the payload logs its coercion.
(() => {
  const events = [];
  const payload = { [Symbol.toPrimitive]() { events.push('coerce'); return 5; } };
  globalThis.input = new Proxy({ value: payload }, { get(object, key) { events.push('get:' + String(key)); return Reflect.get(object, key); } });
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
