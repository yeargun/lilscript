// Harness of the source test: keep() retains the rest callback; after the program the test called
// callback(1,2,3) and printed the observe() log joined with ','.
// The test selected the callback by `value.length >= 3`, which relied on the old route promoting the
// rest wrapper to three formals. docs/language-v0.1.md fixes the length of a JS.methodRest wrapper
// at 0, so this prelude keeps the last kept function (the callback is kept last) instead.
(() => {
  let callback;
  const seen = [];
  globalThis.keep = function (value) { if (typeof value === 'function') callback = value; };
  globalThis.observe = function (a, b, c) { seen.push([a, b, c].join(':')); };
  process.on('exit', () => { callback(1, 2, 3); console.log(seen.join(',')); });
})();
