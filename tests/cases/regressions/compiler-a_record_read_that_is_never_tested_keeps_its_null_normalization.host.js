// Host of compiler.rs a_record_read_that_is_never_tested_keeps_its_null_normalization: the probe
// `seed('a');process.stdout.write([String(peek('a')),String(peek('zz'))].join(':'))`.
{
  globalThis.check = (seed, peek) => {
    seed('a');
    console.log([String(peek('a')), String(peek('zz'))].join(':'));
  };
}
