const factor = 6;

function increment(value) {
  return value + 1 | 0;
}

function threeSteps(value) {
  return increment(increment(increment(value)));
}

function repeated(value) {
  const first = (value * 7) | 0;
  const second = (7 * value) | 0;
  return first + second | 0;
}

function unused(value) {
  return (value * 1000) | 0;
}

class Box {
  constructor(value) {
    this.value = value;
  }

  plus(amount) {
    return this.value + amount | 0;
  }
}

const box = new Box(40);
console.log(threeSteps(1));
console.log(repeated(factor));
console.log(box.plus(2));
console.log("application-build-identifier");
console.log("application-build-identifier");
console.log("application-build-identifier");

if (false) {
  console.log(unused(9));
}
