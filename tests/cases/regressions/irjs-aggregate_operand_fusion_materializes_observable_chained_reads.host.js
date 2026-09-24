// Harness of the source test: each tuple's add() logs its argument; the test printed the log joined with ','.
(() => {
  const hits = [];
  globalThis.test = function () { return true; };
  globalThis.rows = function () {
    return [[0, 0, 0, { add(value) { hits.push(value); } }], [0, 0, 0, { add(value) { hits.push(value); } }], [0, 0, 0, { add(value) { hits.push(value); } }]];
  };
  globalThis.candidate = function () { return 7; };
  globalThis.run = function (callback) { callback({ notifyWith: null }); };
  process.on('exit', () => { console.log(hits.join(',')); });
})();
