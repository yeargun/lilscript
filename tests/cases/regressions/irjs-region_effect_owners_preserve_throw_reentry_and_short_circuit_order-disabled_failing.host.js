// Harness of the source test (enabled=false, fail=true, seed=7): host calls and re-entrant reads
// of `calls` are logged; a throw from next() was caught around the program as 'throw:'+message.
(() => {
  let getCalls;
  const events = [];
  globalThis.observe = function (f) { getCalls = f; };
  globalThis.enabled = function () { events.push('enabled'); return false; };
  globalThis.read = function () { events.push('read'); return 7; };
  globalThis.next = function (previous) { events.push('next:' + previous, 'reenter:' + getCalls()); if (true && previous == 1) throw Error('stop'); return previous + 1; };
  process.on('uncaughtException', (error) => { events.push('throw:' + error.message); });
  process.on('exit', () => { console.log('TRACE:' + events.join(',') + ';calls:' + getCalls()); });
})();
