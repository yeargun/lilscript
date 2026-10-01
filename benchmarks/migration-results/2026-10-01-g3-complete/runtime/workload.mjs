export function run(api, iterations) {
  const box = new api.Box(0);
  let total = 0;
  for (let i=0; i<iterations; i++) {
    box.amount=i&1023;
    total+=box.total();
  }
  return {oracle:total,counters:{calls:iterations,receiver_reads:12*iterations},retained:box};
}
