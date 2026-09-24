// Harness of the source test (a = 0): a proxy logs every key read.
(() => {
  const events = [];
  const target = { a: 0, b: 7 };
  const proxy = new Proxy(target, { get(object, key) { events.push(key); return object[key]; } });
  globalThis.input = function () { return proxy; };
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
