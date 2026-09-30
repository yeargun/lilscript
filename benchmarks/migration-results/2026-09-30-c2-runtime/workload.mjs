export function run(api, iterations) {
  let total = 0;
  for (let seed = 0; seed < iterations; seed++) total += api.score(seed);
  return { oracle: total, counters: { calls: iterations } };
}
