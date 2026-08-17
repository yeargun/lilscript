const values = [4, 9, 16, 25, 36, 49];
let lo = values[0];
let hi = values[0];
for (let i = 1; i < values.length; i++) {
  if (values[i] < lo) lo = values[i];
  if (values[i] > hi) hi = values[i];
}
console.log(lo);
console.log(hi);
