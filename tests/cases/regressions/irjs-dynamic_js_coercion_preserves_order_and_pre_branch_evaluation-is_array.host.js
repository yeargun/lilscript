// Harness of the source test: Array.isArray on a revoked proxy throws; the test caught it around the program.
(() => {
  const events = [];
  const pair = Proxy.revocable([], {});
  const value = pair.proxy;
  pair.revoke();
  globalThis.item = function () { return value; };
  globalThis.flag = function () { return false; };
  process.on('uncaughtException', (error) => { events.push('throw'); });
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
