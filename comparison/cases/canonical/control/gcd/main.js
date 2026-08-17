function gcd(left, right) {
  while (right !== 0) {
    const next = left % right | 0;
    left = right;
    right = next;
  }
  return left;
}
console.log(gcd(99, 11));
