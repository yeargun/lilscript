(() => {
  const wrappers = [];
  globalThis.inspect = function (value) { wrappers.push(value); };
  process.on('exit', () => {
    let [first, second] = wrappers, marker = { ok: true }, constructible = true;
    try { Reflect.construct(first, []); } catch { constructible = false; }
    console.log('TRACE:' + [first !== second, first.length, first.name === '', Object.hasOwn(first, 'prototype'), constructible, first.call(marker) === marker].join(':'));
  });
})();
