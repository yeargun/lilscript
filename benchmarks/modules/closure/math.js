export function weight(value, index) {
  return ((value * ((index + 3) | 0) | 0) + index) | 0;
}

export function unusedPolynomial(value) {
  return ((((value * value) | 0) * value | 0) + 77) | 0;
}
