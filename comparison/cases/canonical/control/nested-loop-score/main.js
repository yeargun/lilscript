function score(limit) {
  let total = 0;
  for (let outer = 0; outer < limit; outer++) {
    if (outer % 3 === 0) continue;
    let inner = 0;
    while (inner < 4) {
      if ((outer + inner) % 2 === 0) {
        total = total + (outer * inner | 0) | 0;
      } else {
        total = total + 1 | 0;
      }
      inner++;
    }
  }
  return total;
}
console.log(score(12));
