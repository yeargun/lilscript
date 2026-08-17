function add(left, right, extra) {
  if (right === void 0) right = 1;
  if (extra === void 0) extra = 0;
  return left + right + extra | 0;
}
console.log(add(7));
console.log(add(7, 2));
console.log(add(7, 2, 3));
