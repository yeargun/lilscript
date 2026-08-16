import { evaluateWindow } from "./rules.js";

function runPolicy() {
  let total = 0;
  const count = algorithmCount();
  for (let index = 0; index + 2 < count; index += 3) {
    total = total + evaluateWindow(
      algorithmInt(index),
      algorithmInt(index + 1),
      algorithmInt(index + 2),
    ) | 0;
  }
  return total;
}

console.log(runPolicy());
