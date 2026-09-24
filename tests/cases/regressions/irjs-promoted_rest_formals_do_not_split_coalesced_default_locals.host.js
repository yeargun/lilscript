// Harness of the source test: keep() retains the callback; after the program the test called
// callback(7, options) and printed [result, options.seen].join(':').
(() => {
  let callback;
  globalThis.keep = function (value) { callback = value; };
  process.on('exit', () => {
    let options = {};
    console.log([callback(7, options), options.seen].join(':'));
  });
})();
