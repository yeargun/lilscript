(() => {
  let seen = null;
  globalThis.limit = function () { return 7; };
  globalThis.consume = function (value) { seen = value; };
  process.on('exit', () => { console.log(JSON.stringify(seen)); });
})();
