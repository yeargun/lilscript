function makeAdder(base) {
  return (value) => base + value | 0;
}
const add = makeAdder(10);
console.log(add(3));
console.log(add(8));
