// Harness of the source test: a proxy logs the for-in enumeration traps.
(() => {
  const events = [];
  const proxy = new Proxy({ x: 1 }, {
    ownKeys(object) { events.push('keys'); return Reflect.ownKeys(object); },
    getOwnPropertyDescriptor(object, key) { events.push('descriptor'); return Reflect.getOwnPropertyDescriptor(object, key); },
  });
  globalThis.input = function () { return proxy; };
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
