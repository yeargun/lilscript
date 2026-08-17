function scan(limit, skip, stop) {
  let total = 0;
  for (let i = 0; i < limit; i++) {
    if (i === skip) continue;
    if (i === stop) break;
    let j = 0;
    while (j < 3) {
      total = total + (i + j | 0) | 0;
      j++;
    }
  }
  return total;
}
console.log(scan(12, 2, 9));
