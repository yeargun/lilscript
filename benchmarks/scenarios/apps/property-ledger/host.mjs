globalThis.consume = (entry) => {
  let total = 0;
  for (const value of Object.values(entry)) total += value;
  return total;
};
