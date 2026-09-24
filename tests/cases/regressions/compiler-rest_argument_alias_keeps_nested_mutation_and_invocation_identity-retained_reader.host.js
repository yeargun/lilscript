// Host of src/compiler_rest_capture_tests.rs `observe(javascript, retained_reader = true)`.
{
  const retained_reader = true;
  let calls = 0, slots = 0, previous, duringSecond;
  globalThis.slot = () => { slots++; return 1 };
  globalThis.invoke = callback => {
    calls++;
    if (retained_reader && calls === 3) duringSecond = previous();
    return callback();
  };
  globalThis.drive = make => {
    const outer = { 1: 'outer' }, first = { id: 1 }, second = { id: 2 };
    const [method, enclosing] = make(outer);
    const a = method('ignored', first);
    if (retained_reader) previous = a[1];
    const b = method('ignored', second);
    const observations = retained_reader
      ? [a[0] === first, b[0] === second, a[1]() === null, b[1]() === null, duringSecond === null]
      : [a === first, b === second];
    console.log(JSON.stringify([...observations, enclosing() === outer, calls, slots]));
  };
}
