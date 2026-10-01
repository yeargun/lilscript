function score(item) {
  return (item.id * 10 | 0) + item.weight | 0;
}
let total = 0;
for (let i = 0; i < 2; i++) {
  total = total + score({id: i + 1, weight: (i * 3 | 0) + 2 | 0}) | 0;
}
console.log(total);
