// Harness of the source test: each() visits every element; consume() logs leaves; the test printed String(seen).
(() => {
  const seen = [];
  globalThis.each = function (list, visit) { for (let i = 0; i < list.length; i++) visit(i, list[i]); };
  globalThis.consume = function (value) { seen.push(value); };
  globalThis.nestedItems = [[1], [2, 3], 4];
  process.on('exit', () => { console.log(String(seen)); });
})();
