function factorial(n) {
  let total = 1;
  for (let i = 2; i <= n; i++) {
    total = total * i | 0;
  }
  return total;
}
console.log(factorial(8));
