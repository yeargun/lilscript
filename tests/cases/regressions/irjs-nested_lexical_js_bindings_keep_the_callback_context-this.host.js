(() => {
  let ambientThis;
  const values = [];
  globalThis.ambient = function (value) { ambientThis = value; };
  globalThis.collect = function (value) { values.push(value); };
  process.on('exit', () => {
    let wrapper = values.shift(), marker = { marker: true };
    wrapper.call(marker);
    let nested = values.shift();
    console.log('TRACE:' + [nested() === ambientThis, nested() !== marker].join(':'));
  });
})();
