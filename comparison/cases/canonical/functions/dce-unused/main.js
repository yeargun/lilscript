function used(value) {
  return value + 2 | 0;
}
function unused(value) {
  return value * 1000 | 0;
}
console.log(used(6));
