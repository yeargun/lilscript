import { beginSummary, makeLine, appendSummary, finishSummary } from "./model.js";

function runInvoice() {
  let summary = beginSummary();
  const count = algorithmCount();
  for (let index = 0; index + 3 < count; index += 4) {
    summary = appendSummary(summary, makeLine(
      algorithmInt(index),
      algorithmInt(index + 1),
      algorithmInt(index + 2),
      algorithmInt(index + 3),
    ));
  }
  return finishSummary(summary);
}

console.log(runInvoice());
