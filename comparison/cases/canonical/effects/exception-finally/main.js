function guarded(start) {
  let value = start;
  try {
    value = value + 2 | 0;
    throw value;
  } catch (err) {
    value = value + 3 | 0;
  } finally {
    value = value + 4 | 0;
  }
  return value;
}
console.log(guarded(1));
