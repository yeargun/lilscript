function measure(seed) {
  const prefix = [seed, (seed + 1) | 0];
  const values = [...prefix, (seed + 2) | 0, (seed + 3) | 0];
  const first = values[0] ?? null;
  const second = values[1] ?? null;
  const tail = values.slice(2);
  let total = ((first ?? 0) + (second ?? 0)) | 0;
  for (let index = 0; index < tail.length; index = (index + 1) | 0) {
    total = (total + tail[index]) | 0;
  }
  const merged = {
    __proto__: null,
    ...{
      __proto__: null,
      a: seed,
      b: (seed + 1) | 0,
      c: (seed + 2) | 0,
      d: (seed + 3) | 0,
    },
    b: (seed + 4) | 0,
    e: (seed + 5) | 0,
  };
  const a = merged.a ?? null;
  const b = merged.b ?? null;
  const remaining = { __proto__: null, ...merged };
  delete remaining.a;
  delete remaining.b;
  total = (total + (a ?? 0)) | 0;
  total = (total + (b ?? 0)) | 0;
  total = (total + (remaining.c ?? 0)) | 0;
  total = (total + (remaining.d ?? 0)) | 0;
  return (total + (remaining.e ?? 0)) | 0;
}
let total = 0;
for (let i = 0; i < 4000; i++) total = (total + measure(i % 97)) | 0;
console.log(total);
