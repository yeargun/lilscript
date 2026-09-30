export function score(seed) {
  let total = 0;
  for (let index = 0; index < 32; index++) total += (seed + index) & 255;
  return total;
}
