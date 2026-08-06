import levenshtein from "js-levenshtein";

const left = [
  "a", "ab", "ac", "abc", "kitten", "xabxcdxxefxgx", "cat",
  "xabxcdxxefxgx", "javawasneat", "example", "sturgeon",
  "levenshtein", "distance", "因為我是中國人所以我會說中文",
];
const right = [
  "b", "ac", "bc", "axc", "sitting", "1ab2cd34ef5g6", "cow",
  "abcdefg", "scalaisgreat", "samples", "urgently", "frankenstein",
  "difference", "因為我是英國人所以我會說英文",
];
const expected = [1, 1, 1, 1, 3, 6, 2, 6, 7, 3, 6, 6, 5, 2];

let passed = 0;
for (let index = 0; index < left.length; index += 1) {
  if (levenshtein(left[index], right[index]) === expected[index]) passed += 1;
}

let digest = 0;
for (let iteration = 0; iteration < 50_000; iteration += 1) {
  const first = iteration % left.length;
  const second = (iteration * 5 + 3) % right.length;
  digest += levenshtein(left[first], right[second]) * ((iteration % 7) + 1);
}
console.log(`js-levenshtein:${passed}:${digest}`);
