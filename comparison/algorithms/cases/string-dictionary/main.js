// Quoted keys are part of the dynamic dictionary ABI and remain stable in
// property-renaming toolchains such as Closure Compiler ADVANCED.
const keywords = { "alpha": 11, "beta": 17, "gamma": 23, "delta": 29, "epsilon": 31 };

function keywordCode(value) {
  return Object.hasOwn(keywords, value) ? keywords[value] : 3;
}

function prefixCode(value) {
  return value.startsWith("pre-") ? 7
    : value.startsWith("post-") ? 13
    : value.startsWith("internal-") ? 19
    : 1;
}

function tokenScore(value, index) {
  return keywordCode(value) + prefixCode(value) + value.length * (index + 1) | 0;
}

function scoreLabel(score) {
  const group = (score % 4 + 4) % 4;
  return [
    "dictionary-even-zero",
    "dictionary-even-one",
    "dictionary-odd-two",
    "dictionary-odd-three",
  ][group];
}

function dictionaryScore() {
  let total = 0;
  for (let index = 0; index < algorithmCount(); index++) {
    total = total + tokenScore(algorithmString(index), index) | 0;
  }
  console.log(scoreLabel(total));
  return total;
}

console.log(dictionaryScore());
