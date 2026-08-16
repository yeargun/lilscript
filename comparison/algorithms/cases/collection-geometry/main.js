function makePoint(x, y) {
  return { _x: x, _y: y };
}

function translate(point, dx, dy) {
  return { _x: point._x + dx | 0, _y: point._y + dy | 0 };
}

function energy(point) {
  return (point._x * point._x | 0) + (point._y * point._y | 0) | 0;
}

function cross(left, right) {
  return (left._x * right._y | 0) - (left._y * right._x | 0) | 0;
}

function collectValues() {
  const values = [];
  for (let index = 0; index < algorithmCount(); index++) values.push(algorithmInt(index));
  return values;
}

function analyzeGeometry() {
  const values = collectValues();
  let total = 0;
  for (let index = 0; index + 3 < values.length; index += 4) {
    const first = translate(makePoint(values[index], values[index + 1]), index, -index);
    const second = makePoint(values[index + 2], values[index + 3]);
    total = total + energy(first) + cross(first, second) | 0;
  }
  return total;
}

console.log(analyzeGeometry());
