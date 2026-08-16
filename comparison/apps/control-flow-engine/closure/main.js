function score(limit) {
  let total = 0;
  for (let outer = 0; outer < limit; outer = outer + 1 | 0) {
    if ((outer % 3 | 0) === 0) continue;
    let inner = 0;
    while (inner < 4) {
      if (((outer + inner | 0) % 2 | 0) === 0) {
        total = total + (outer * inner | 0) | 0;
      } else {
        total = total + 1 | 0;
      }
      inner = inner + 1 | 0;
    }
  }
  return total;
}

console.log(score(12));
