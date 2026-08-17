const values = [2, 7, 1, 8, 2, 8];
let total = 0;
for (let i = 0; i < values.length; i++) {
  total = total + values[i] | 0;
  console.log(total);
}
