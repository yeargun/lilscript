import emotionHash from "@emotion/hash";

const values = ["something", "", "a", "color: hotpink;", "display:grid", "A😀Z", "café", "中文网页"];
const expected = ["crsxd7", "0", "acqbnw", "1bh9win", "vetbs0", "13a52ga", "1sy5lhz", "1gn5who"];
let passed = 0;
for (let index = 0; index < values.length; index += 1) {
  if (emotionHash(values[index]) === expected[index]) passed += 1;
}

let digest = 0;
for (let iteration = 0; iteration < 100_000; iteration += 1) {
  const hash = emotionHash(values[iteration % values.length]);
  digest += hash.length * 37 + hash.charCodeAt(iteration % hash.length);
}
console.log(`emotion-hash:${passed}:${digest}`);
