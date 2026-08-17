function churn(value, factor, offset, rounds) {
  for (let i = 0; i < rounds; i++) {
    value = (value * factor | 0) + offset | 0;
  }
  return value;
}
const value = churn(123456789, 1664525, 1013904223, 3);
console.log(value);
console.log(value / 257 | 0);
console.log(value % 257 | 0);
console.log(value >>> 5);
