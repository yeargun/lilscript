function unused(value) {
  return value * 1000 | 0;
}
console.log(4);
if (false) {
  console.log(unused(9));
}
