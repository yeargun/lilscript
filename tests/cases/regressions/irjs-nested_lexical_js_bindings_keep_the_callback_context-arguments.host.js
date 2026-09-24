(() => {
  const values = [];
  globalThis.collect = function (value) { values.push(value); };
  process.on('exit', () => {
    let wrapper = values.shift(), wrapperArguments = wrapper(4, 5), nested = values.shift(), captured = nested();
    console.log('TRACE:' + [captured[0], wrapperArguments[0], captured !== wrapperArguments].join(':'));
  });
})();
