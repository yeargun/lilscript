function score(point) {
  return point.x * 3 + point.y * 5 | 0;
}
const point = {x: 3, y: 4};
console.log(score(point));
