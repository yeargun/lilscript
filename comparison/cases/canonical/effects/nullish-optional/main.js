let calls = 0;
function nextIndex() {
  calls++;
  return 0;
}
const values = [8, 9];
console.log(values?.[nextIndex()] ?? 4);
console.log(calls);
