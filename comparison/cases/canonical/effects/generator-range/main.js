function* range(start, stop, step) {
  for (let value = start; value < stop; value = value + step | 0) yield value;
}
let total = 0;
let count = 0;
for (const value of range(1, 7, 2)) {
  total = total + value | 0;
  count++;
}
console.log(total);
console.log(count);
