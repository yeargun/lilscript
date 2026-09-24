// Host of compiler.rs object_has_own_direct_and_detached_calls_preserve_runtime_order:
// `let trace=[];function nextObject(){trace.push('object');return {owned:1}}
//  function nextKey(){trace.push('key');return 'owned'}`, and after the program
// `process.stdout.write('trace='+trace.join(','))`. The harness defines no `objectHasOwn`.
{
  const trace = [];
  globalThis.nextObject = function nextObject() { trace.push('object'); return { owned: 1 }; };
  globalThis.nextKey = function nextKey() { trace.push('key'); return 'owned'; };
  queueMicrotask(() => console.log('trace=' + trace.join(',')));
}
