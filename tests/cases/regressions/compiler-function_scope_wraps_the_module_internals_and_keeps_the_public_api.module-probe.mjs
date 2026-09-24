// Probe of compiler.rs function_scope_wraps_the_module_internals_and_keeps_the_public_api:
// `[look('a'),look('b'),look('a'),missed(),look.name===missed.name,look.length,typeof look.prototype]`.
// The test asserts the prefix "a!:b!:a!:2:false:1:" (plain and function-scope builds agree); the
// last element (`typeof look.prototype`) had no asserted value and is omitted here.
export default function probe(m) {
  const { look, missed } = m;
  console.log([look('a'), look('b'), look('a'), missed(), look.name === missed.name, look.length].join(':'));
}
