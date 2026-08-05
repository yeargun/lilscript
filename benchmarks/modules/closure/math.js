export function weight(value, index) {
  return (Math.imul(value, (index + 3) | 0) + index) | 0;
}

export function unusedPolynomial(value) {
  return (Math.imul(Math.imul(value, value), value) + 77) | 0;
}
