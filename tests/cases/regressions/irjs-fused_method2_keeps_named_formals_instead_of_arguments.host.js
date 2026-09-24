// Harness of the source test: the consumed function is kept, other values logged; the test then
// called seen.call({ok:1},'n','v').
(() => {
  let seen = null;
  const props = [];
  globalThis.consume = function (value) { if (typeof value === 'function') { seen = value; } else props.push(value); };
  process.on('exit', () => { console.log([seen.call({ ok: 1 }, 'n', 'v').ok, props.join('')].join(':')); });
})();
