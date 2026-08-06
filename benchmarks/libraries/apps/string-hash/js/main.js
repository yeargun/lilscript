import stringHash from "string-hash";

let passed = 0;
if (stringHash("Mary had a little lamb.") === 1766333550) passed += 1;
if (stringHash("Hello, world!") === 343662184) passed += 1;
if (stringHash("A😀Z") === 2106584963) passed += 1;
if (stringHash("café") === 2083234952) passed += 1;

const values = ["Mary had a little lamb.", "Hello, world!", "LilScript", "A😀Z", "café"];
let digest = 0;
for (let index = 0; index < 90_000; index += 1) {
  digest = (digest + stringHash(values[index % values.length])) % 4294967291;
}
console.log(`string-hash:${passed}:${digest}`);
