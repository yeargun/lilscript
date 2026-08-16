import { combine, finalize } from "./helpers.js";

function runHelpers() {
  let total = 0;
  const count = algorithmCount();
  for (let index = 0; index < count; index++) {
    total = total + combine(algorithmInt(index), index) | 0;
  }
  return finalize(total, count);
}

console.log(runHelpers());
