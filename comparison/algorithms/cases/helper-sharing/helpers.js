function affine(value, multiplier, offset) {
  return (value * multiplier | 0) + offset | 0;
}

function forward(value) {
  return affine(value, 3, 7);
}

function backward(value) {
  return affine(value, -2, 11);
}

function foldLeft(value, index) {
  return forward(value) ^ affine(index, 2, 3);
}

function foldRight(value, index) {
  return backward(value + index | 0) ^ affine(index, 2, 3);
}

export function combine(value, index) {
  return foldLeft(value, index) ^ foldRight(value, index);
}

export function finalize(total, count) {
  return affine(total + (count * 13 | 0) | 0, 5, count * -52 | 0);
}
