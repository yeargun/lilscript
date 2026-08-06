function area(rectangle) {
  return (rectangle.width * rectangle.height) | 0;
}

class ModelCounter {
  constructor(initial) {
    this.value = initial;
  }

  add(amount) {
    this.value = this.value + amount | 0;
    return this.value;
  }
}

const rectangle = {
  origin: {x: 3, y: 4},
  width: 6,
  height: 7,
};
const counter = new ModelCounter(rectangle.origin.x + rectangle.origin.y | 0);
console.log(area(rectangle));
console.log(counter.add(rectangle.width));
console.log(counter.add(rectangle.height));
