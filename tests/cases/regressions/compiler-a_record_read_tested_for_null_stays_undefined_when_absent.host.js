// Host of compiler.rs a_record_read_tested_for_null_stays_undefined_when_absent: the probe
// `process.stdout.write([look('a'),look('a'),String(raw('a')),String(raw('zz'))].join(':'))`.
{
  globalThis.check = (look, raw) => {
    console.log([look('a'), look('a'), String(raw('a')), String(raw('zz'))].join(':'));
  };
}
