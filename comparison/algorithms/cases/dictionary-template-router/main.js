import { tokenScore, labelFor, renderDigest } from "./dictionary.js";

function runDictionaryRouter() {
  let total = 0;
  const count = algorithmCount();
  for (let index = 0; index < count; index++) {
    total = total + tokenScore(algorithmString(index), index) | 0;
  }
  console.log(labelFor(total));
  return renderDigest(total, count);
}

console.log(runDictionaryRouter());
