function area(rect) {
  return rect.width * rect.height | 0;
}
const rect = {origin: {x: 1, y: 2}, width: 6, height: 7};
console.log(area(rect));
console.log(rect.origin.x + rect.origin.y | 0);
