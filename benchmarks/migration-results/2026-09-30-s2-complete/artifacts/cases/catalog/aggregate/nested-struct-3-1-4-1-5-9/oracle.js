function area(rect) {
  return ((rect.width * rect.height | 0) + rect.origin.x | 0) + rect.origin.y | 0;
}
const values = [3, 1, 4, 1, 5, 9];
let total = 0;
for (let i = 0; i + 3 < values.length; i += 4) {
  const w = values[i + 2] === 0 ? 1 : values[i + 2];
  const h = values[i + 3] === 0 ? 1 : values[i + 3];
  total = total + area({origin: {x: values[i], y: values[i + 1]}, width: w, height: h}) | 0;
}
console.log(total);
