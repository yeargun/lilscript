function score(kind) {
  if (kind === 0) return 1;
  if (kind === 1) return 2;
  return 4;
}
console.log(score(1));
console.log(score(2));
