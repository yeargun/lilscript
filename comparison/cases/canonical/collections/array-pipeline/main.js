const values = [1, 2, 3, 4, 5];
const mapped = values.map(value => value * 2 | 0);
const selected = mapped.filter(value => value % 4 === 0);
console.log(selected.reduce((total, value) => total + value | 0, 0));
