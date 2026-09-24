// Harness of the source test: identity, non-empty name, length and a direct call of the wrapper.
(() => {
  const values = [];
  globalThis.consume = function (value) { values.push(value); };
  process.on('exit', () => {
    let value = values[0];
    console.log('TRACE:' + [values[0] === values[1], value.name !== '', value.length, value(9)].join(':'));
  });
})();
