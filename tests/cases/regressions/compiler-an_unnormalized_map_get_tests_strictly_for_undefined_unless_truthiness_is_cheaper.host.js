// Host of compiler.rs an_unnormalized_map_get_tests_strictly_for_undefined_unless_truthiness_is_cheaper:
// the probe `process.stdout.write([count('a'),count('b'),count('a'),count('a')].join(':'))`.
{
  globalThis.check = count => {
    console.log([count('a'), count('b'), count('a'), count('a')].join(':'));
  };
}
