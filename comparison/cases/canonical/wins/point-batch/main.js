function manhattan(point) {
  let x = point.x;
  if (x < 0) x = -x | 0;
  let y = point.y;
  if (y < 0) y = -y | 0;
  return x + y | 0;
}
const values = [3, -4, 5, 6, -7, 8, 1, -2];
let total = 0;
for (let i = 0; i + 1 < values.length; i += 2) {
  total = total + manhattan({x: values[i], y: values[i + 1]}) | 0;
}
console.log(total);
