const values = [3, 1, 4, 1, 5, 9];
let total = 0;
for (let i = 0; i < values.length; i++) {
  total = total + values[i] | 0;
}
console.log(total);
