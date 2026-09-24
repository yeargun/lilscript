// Host of compiler.rs owned_plain_object_proof_forwards_only_proven_own_reads (first program):
// `let calls=0;function read(){calls++;return 7}`, and after the program `process.stdout.write(String(calls))`.
{
  let calls = 0;
  globalThis.read = function read() { calls++; return 7; };
  queueMicrotask(() => console.log(String(calls)));
}
