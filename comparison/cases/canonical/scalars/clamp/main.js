function clamp(value, lo, hi) {
  if (value < lo) return lo;
  if (value > hi) return hi;
  return value;
}
console.log(clamp(3, 0, 10));
console.log(clamp(11, 0, 10));
console.log(clamp(-2, 0, 10));
