// Harness of the source test: identity, anonymous name, length, own prototype and constructibility
// of the consumed wrapper.
(() => {
  const values = [];
  globalThis.barrier = function () {};
  globalThis.consume = function (value) { values.push(value); };
  process.on('exit', () => {
    let value = values[0], constructible = true;
    try { Reflect.construct(value, []); } catch { constructible = false; }
    console.log('TRACE:' + [values[0] === values[1], value.name, value.length, Object.hasOwn(value, 'prototype'), constructible].join(':'));
  });
})();
