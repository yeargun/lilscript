// Harness of the source test: after the program it called the wrapper with a marker receiver and
// recorded [result===null, anonymous name, length].
(() => {
  const values = [];
  globalThis.collect = function (value) { values.push(value); };
  process.on('exit', () => {
    let wrapper = values[0], marker = { ok: true };
    console.log('TRACE:' + [wrapper.call(marker) === null, wrapper.name === '', wrapper.length].join(':'));
  });
})();
