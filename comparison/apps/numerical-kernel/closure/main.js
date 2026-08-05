function factorial(value) {
  if (value <= 1) return 1;
  return Math.imul(value, factorial(value - 1 | 0));
}

function gcd(left, right) {
  while (right !== 0) {
    const remainder = left % right | 0;
    left = right;
    right = remainder;
  }
  return left;
}

function fibonacci(count) {
  let previous = 0;
  let current = 1;
  for (let index = 0; index < count; index = index + 1 | 0) {
    const next = previous + current | 0;
    previous = current;
    current = next;
  }
  return previous;
}

console.log(factorial(7));
console.log(gcd(1071, 462));
console.log(fibonacci(12));
