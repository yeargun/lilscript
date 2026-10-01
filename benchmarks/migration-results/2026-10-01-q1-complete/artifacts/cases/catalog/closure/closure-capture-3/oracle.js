function run() {
  const factor = 3;
const scale = (value) => value * factor | 0;
let total = 0;
for (let i = 1; i <= 6; i++) {
  total = total + scale(i) | 0;
}
  return total;
}
console.log(run());
