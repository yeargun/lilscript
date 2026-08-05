class Vector {
  constructor(x, y) {
    this.x = x;
    this.y = y;
  }

  lengthSquared() {
    return this.x * this.x + this.y * this.y;
  }
}

const values = [1, 2, 3, 4];
const doubled = values.map((value) => Math.imul(value, 2));
let sum = 0;

for (let index = 0; index < doubled.length; index = (index + 1) | 0) {
  sum = (sum + doubled[index]) | 0;
}

const vector = new Vector(3, 4);
if (vector.lengthSquared() === 25) {
  console.log(`sum=${sum}`);
} else {
  console.log("invalid");
}

