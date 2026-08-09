import {
  customAlphabet,
  customRandom,
  nanoid,
  random,
  urlAlphabet,
} from "./api.js";

let passed = 0;
if (urlAlphabet.length === 64) passed += 1;
if (nanoid().length === 21) passed += 1;
if (nanoid(21).length === 21) passed += 1;
if (nanoid(10).length === 10) passed += 1;

const abc = customAlphabet("abcdefghijklmnopqrstuvwxyz", 5);
if (abc(5).length === 5) passed += 1;
if (abc(7).length === 7) passed += 1;

const fixed = customRandom("ab", 4, (size) => {
  const bytes = new Uint8Array(size);
  bytes.fill(1);
  return bytes;
});
if (fixed(8).length === 8) passed += 1;
if (random(3).length === 3) passed += 1;

console.log(`nanoid:${passed}:${urlAlphabet.length}`);
