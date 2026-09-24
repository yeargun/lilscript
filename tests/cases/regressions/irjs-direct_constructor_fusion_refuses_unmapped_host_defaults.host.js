// Harness of the source test: the global Map is replaced by a logging class.
(() => {
  const events = [];
  globalThis.Map = class { constructor() { events.push('map'); } };
  globalThis.read = function () { events.push('read'); return 7; };
  globalThis.inspect = function (cache) { events.push('inspect:' + cache[1]); };
  process.on('exit', () => { console.log('TRACE:' + events.join(',')); });
})();
