// Harness of the source test: inspect records JSON.stringify(value); the test printed the records joined with '\n'.
(() => {
  const values = [];
  globalThis.inspect = function (value) { values.push(JSON.stringify(value)); };
  process.on('exit', () => { console.log(values.join('\n')); });
})();
