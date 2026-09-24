// Host of compiler.rs owned_plain_object_proof_forwards_only_proven_own_reads (third program):
// `function escape(value){Object.defineProperty(value,'alpha',{get(){return 11}})}`.
{
  globalThis.escape = function escape(value) {
    Object.defineProperty(value, 'alpha', { get() { return 11; } });
  };
}
