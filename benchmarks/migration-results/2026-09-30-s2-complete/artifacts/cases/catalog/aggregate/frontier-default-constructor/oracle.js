class Counter {
  constructor() { this.value = 0; }
  increment() {
    this.value += 1;
    return this.value;
  }
}
function bump(counter = new Counter()) {
  return counter.increment();
}
console.log(bump());
console.log(bump());
