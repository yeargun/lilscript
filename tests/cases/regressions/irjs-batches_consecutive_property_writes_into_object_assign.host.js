(() => {
  let seen = null;
  globalThis.input = function () { return {}; };
  globalThis.consume = function (value) { seen = value; };
  process.on('exit', () => { console.log(JSON.stringify(seen)); });
})();
