// Harness of the source test: expose() installs an own `push` on the escaped array;
// JS.push must still call Array.prototype.push. The test printed 'TRACE:'+touched after the program.
(() => {
  let touched = false;
  globalThis.read = function () { return 1; };
  globalThis.expose = function (value) { value.push = () => { touched = true; }; };
  process.on('exit', () => { console.log('TRACE:' + touched); });
})();
