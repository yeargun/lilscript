async function resolveValue(value) {
  const resolved = await Promise.resolve(value + 5 | 0);
  return resolved * 2 | 0;
}
resolveValue(3).then(value => console.log(value));
