// Harness of the source test: proxies log the read of input and the write to target.
(() => {
  const events = [];
  globalThis.input = new Proxy({ value: 9 }, { get(object, key) { events.push('get:' + String(key)); return Reflect.get(object, key); } });
  globalThis.target = new Proxy({}, { set(object, key, value) { events.push('set:' + String(key) + ':' + value); return Reflect.set(object, key, value); } });
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
