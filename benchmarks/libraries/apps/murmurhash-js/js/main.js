import murmur from "murmurhash-js";

const values = ["", "a", "hello", "LilScript", "color:hotpink", "0123456789abcdef"];
const seeds = [0, 1, 42, 123456789, 2147483647, 7];
const expected2 = [0, 626045324, 2013460684, 3301369031, 1739108709, 2343271958];
const expected3 = [0, 1485495528, 3806057185, 2367583532, 3695485312, 1467914068];
let passed = 0;
for (let index = 0; index < values.length; index += 1) {
  if (murmur.murmur2(values[index], seeds[index]) === expected2[index]) passed += 1;
  if (murmur.murmur3(values[index], seeds[index]) === expected3[index]) passed += 1;
  if (murmur(values[index], seeds[index]) === expected3[index]) passed += 1;
}

let digest = 0;
for (let iteration = 0; iteration < 60_000; iteration += 1) {
  const index = iteration % values.length;
  digest += murmur.murmur2(values[index], seeds[index]);
  digest += murmur.murmur3(values[(index + 3) % values.length], seeds[index]);
  while (digest >= 4_294_967_291) digest -= 4_294_967_291;
}
console.log(`murmurhash-js:${passed}:${digest}`);
