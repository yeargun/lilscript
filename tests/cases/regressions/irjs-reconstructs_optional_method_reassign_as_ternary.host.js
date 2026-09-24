// Harness of the source test: it printed JSON.stringify(q)+':'+(q2[0][1]===obj).
(() => {
  globalThis.firstQueue = [];
  globalThis.firstArgs = [1, 2];
  globalThis.secondQueue = [];
  globalThis.secondArgs = { x: 1 };
  process.on('exit', () => { console.log(JSON.stringify(globalThis.firstQueue) + ':' + (globalThis.secondQueue[0][1] === globalThis.secondArgs)); });
})();
