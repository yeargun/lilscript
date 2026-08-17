class Counter {
  constructor(initial) {
    this.value = initial;
  }
  add(amount) {
    this.value = this.value + amount | 0;
    return this.value;
  }
}
const counter = new Counter(4);
let total = 0;
for (let i = 0; i < 6; i++) {
  total = total + counter.add(i) | 0;
}
console.log(total);
