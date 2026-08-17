function fibonacci(n) {
  let prev = 0;
  let current = 1;
  for (let i = 0; i < n; i++) {
    const next = prev + current | 0;
    prev = current;
    current = next;
  }
  return prev;
}
console.log(fibonacci(12));
