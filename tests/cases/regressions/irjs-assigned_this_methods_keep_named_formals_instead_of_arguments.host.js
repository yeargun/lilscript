// Harness of the source test: the object carrying animate is kept; other values are logged;
// after the program the test called seen.animate.call({ok:1},'p','s','e','c').
(() => {
  let seen = null;
  const props = [];
  globalThis.consume = function (value) { if (value && value.animate) { seen = value; } else props.push(value); };
  process.on('exit', () => { console.log([seen.animate.call({ ok: 1 }, 'p', 's', 'e', 'c').ok, props.join('')].join(':')); });
})();
