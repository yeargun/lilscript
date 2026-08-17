let total = 0;
for (let i = 0; i < 16; i++) {
  const bucket = i % 4 | 0;
  if (bucket === 0) total = total + i | 0;
  else if (bucket === 1) total = total + (i * 2 | 0) | 0;
  else if (bucket === 2) total = total + (i * 3 | 0) | 0;
  else total = total - 1 | 0;
}
console.log(total);
