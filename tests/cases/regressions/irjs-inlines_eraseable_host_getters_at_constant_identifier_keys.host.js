(() => {
  const values = [];
  globalThis.consume = function (value) { values.push(value); };
  process.on('exit', () => { console.log(values.join(':')); });
})();
