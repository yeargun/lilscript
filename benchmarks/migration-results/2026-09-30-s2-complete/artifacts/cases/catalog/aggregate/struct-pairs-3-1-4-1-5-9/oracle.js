function score(point) {
  return (point.x * 3 | 0) + (point.y * 5 | 0) | 0;
}
const values = [3, 1, 4, 1, 5, 9];
let total = 0;
for (let i = 0; i + 1 < values.length; i += 2) {
  total = total + score({x: values[i], y: values[i + 1]}) | 0;
}
console.log(total);
