function factorial(value) {
  if (value <= 1) return 1;
  return value * factorial(value - 1 | 0) | 0;
}
console.log(factorial(8));
