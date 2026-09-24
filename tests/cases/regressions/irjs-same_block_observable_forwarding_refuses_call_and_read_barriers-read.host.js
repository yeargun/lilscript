// Harness of the source test (read barrier): reads and coercions are logged in order.
(() => {
  const events = [];
  const value = label => ({ [Symbol.toPrimitive]() { events.push('coerce:' + label); return label; } });
  globalThis.input = new Proxy({ first: value('first'), second: value('second') }, { get(object, key) { events.push('get:' + String(key)); return Reflect.get(object, key); } });
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
