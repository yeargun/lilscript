// Harness of the source test: freshness, length, anonymous name, own prototype, constructibility,
// receiver and arguments-object behaviour of every adapter wrapper.
(() => {
  const wrappers = [];
  globalThis.inspect = function (value) { wrappers.push(value); };
  process.on('exit', () => {
    let [m0a, m0b, m1a, m1b, m2a, m2b, m3a, m3b, mra, mrb, sra, srb] = wrappers, marker = { ok: true }, rest = mra.call(marker, 4, 5), staticArgs = sra(6, 7), constructible = wrappers.every(value => { try { Reflect.construct(value, []); return true; } catch { return false; } });
    console.log('TRACE:' + [m0a !== m0b, m1a !== m1b, m2a !== m2b, m3a !== m3b, mra !== mrb, sra !== srb, m0a.length, m1a.length, m2a.length, m3a.length, mra.length, sra.length, wrappers.every(value => value.name === ''), wrappers.every(value => Object.hasOwn(value, 'prototype')), constructible, m0a.call(marker) === marker, m1a.call(marker, 9) === marker, m2a.call(marker, 8, 7) === marker, m3a.call(marker, 8, 7, 6) === marker, Object.prototype.toString.call(rest), rest.length, rest[0], rest[1], Object.prototype.toString.call(staticArgs), staticArgs.length, staticArgs[0], staticArgs[1]].join(':'));
  });
})();
