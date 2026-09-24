// Harness of the source test (first = 5): proxies log every read of input and write to the destination.
(() => {
  const events = [];
  globalThis.input = new Proxy({ first: 5, second: 7 }, { get(object, key) { events.push('get:' + key); return Reflect.get(object, key); } });
  const target = {};
  globalThis.destination = function () {
    events.push('destination');
    return new Proxy(target, { set(object, key, value) { events.push('set:' + key + ':' + value); return Reflect.set(object, key, value); } });
  };
  globalThis.property = function () { events.push('property'); return 'result'; };
  process.on('exit', () => { console.log(events.join(',')); });
})();
