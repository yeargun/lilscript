// Host of compiler.rs owned_plain_object_proof_forwards_only_proven_own_reads (second program):
// an inherited `missing` getter returning 9, deleted after the program.
{
  Object.defineProperty(Object.prototype, 'missing', { configurable: true, get() { return 9; } });
  queueMicrotask(() => { delete Object.prototype.missing; });
}
