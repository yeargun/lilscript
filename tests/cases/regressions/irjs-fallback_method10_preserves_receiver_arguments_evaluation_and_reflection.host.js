// Harness of the source test: a method10 wrapper called with eleven arguments evaluates all of them
// in order and passes the first ten.
(() => {
  const wrappers = [];
  globalThis.inspect = function (value) { wrappers.push(value); };
  globalThis.keep = function () {};
  process.on('exit', () => {
    let [first, second] = wrappers, marker = { ok: true }, order = [];
    function next(value) { order.push(value); return value; }
    let result = first.call(marker, next(1), next(2), next(3), next(4), next(5), next(6), next(7), next(8), next(9), next(10), next(11)), constructible = true;
    try { Reflect.construct(first, []); } catch { constructible = false; }
    console.log('TRACE:' + [first !== second, first.length, first.name === '', Object.hasOwn(first, 'prototype'), constructible, result[0] === marker, result.slice(1).join(','), order.join(',')].join(':'));
  });
})();
