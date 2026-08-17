class Scale {
  constructor(factor) {
    this.factor = factor;
  }
  apply(value) {
    return value * this.factor | 0;
  }
}
const scale = new Scale(3);
let total = 0;
for (let i = 1; i <= 8; i++) {
  total = total + scale.apply(i) | 0;
}
console.log(total);
console.log(scale.factor);
