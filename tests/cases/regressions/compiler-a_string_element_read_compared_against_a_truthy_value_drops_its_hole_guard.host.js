// Host of compiler.rs a_string_element_read_compared_against_a_truthy_value_drops_its_hole_guard:
// the probe `process.stdout.write([matched(['a','b'],['a','b']),matched(['a','b'],['a','z']),
// matched(['a','b'],['a']),matched([0,'b'],['b'])].join(':'))`.
{
  globalThis.check = matched => {
    console.log([
      matched(['a', 'b'], ['a', 'b']),
      matched(['a', 'b'], ['a', 'z']),
      matched(['a', 'b'], ['a']),
      matched([0, 'b'], ['b']),
    ].join(':'));
  };
}
