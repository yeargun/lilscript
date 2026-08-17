let calls = 0;
function probe(value) {
  calls = calls + value | 0;
  return value % 2 === 0;
}
const result = probe(1) && probe(2) || probe(4);
console.log(result);
console.log(calls);
