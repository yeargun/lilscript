function firstPositive(values) {
  for (let i = 0; i < values.length; i++) {
    if (values[i] > 0) return values[i];
  }
  return 0;
}
console.log(firstPositive([-2, 0, 5, 9]));
