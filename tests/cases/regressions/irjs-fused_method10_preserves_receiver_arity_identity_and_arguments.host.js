(() => {
  const wrappers = [];
  globalThis.inspect = function (value) { wrappers.push(value); };
  process.on('exit', () => {
    let [first, second] = wrappers, marker = { ok: true }, result = first.call(marker, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10), constructible = true;
    try { Reflect.construct(first, []); } catch { constructible = false; }
    console.log('TRACE:' + [first !== second, first.length, first.name === '', Object.hasOwn(first, 'prototype'), constructible, result[0] === marker, result.slice(1).join(',')].join(':'));
  });
})();
