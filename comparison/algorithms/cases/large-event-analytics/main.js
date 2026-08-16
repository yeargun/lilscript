import { processEvents, renderResult } from "./pipeline.js";

function runEventAnalytics() {
  return renderResult(processEvents());
}

console.log(runEventAnalytics());
