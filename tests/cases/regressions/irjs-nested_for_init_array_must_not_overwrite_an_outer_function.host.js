// Harness of the source test: keep() retains the parser; after the program the test logged
// typeof parser and String(parser()) and printed the log joined with ','.
(() => {
  let parser;
  const seen = [];
  globalThis.keep = function (cb) { parser = cb; };
  globalThis.consume = function (v) { seen.push(Array.isArray(v) ? v.join(':') : typeof v); };
  process.on('exit', () => {
    seen.push(typeof parser);
    if (typeof parser === 'function') seen.push(String(parser()));
    console.log(seen.join(','));
  });
})();
