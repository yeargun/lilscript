// Harness of the source test: keep() retains a kept function, logs other values; after the
// program the test called createProperty('class') and logged info.propName.
(() => {
  let createProperty;
  const seen = [];
  globalThis.keep = function (v) { if (typeof v === 'function') createProperty = v; else seen.push(v + ''); };
  process.on('exit', () => {
    let info = createProperty('class');
    seen.push(info.propName);
    console.log(seen.join(','));
  });
})();
