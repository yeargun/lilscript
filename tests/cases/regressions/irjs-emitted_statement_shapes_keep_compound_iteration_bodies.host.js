(() => {
  const events = [];
  globalThis.limit = function () { return 2; };
  globalThis.object = function () { return { a: 1, b: 2 }; };
  globalThis.event = function (x) { events.push(x); };
  process.on('exit', () => { console.log(events.join(',')); });
})();
