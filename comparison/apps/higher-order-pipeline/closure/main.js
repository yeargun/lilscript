const factor = 3;
const values = [1, 2, 3, 4, 5, 6];
const scaled = values.map((value) => value * factor | 0);
const selected = scaled.filter((value) => (value % 2 | 0) === 0);
const total = selected.reduce((sum, value) => sum + value | 0, 0);
selected.forEach((value) => {
  console.log(value);
});
console.log(total);
