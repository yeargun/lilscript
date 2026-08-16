import { eventScore } from "./score.js";
import { beginAggregate, updateAggregate, finishAggregate } from "./aggregate.js";

export function processEvents() {
  const first = eventScore(algorithmInt(0), algorithmString(0), 0);
  let stats = beginAggregate(first);
  const count = algorithmCount();
  for (let index = 1; index < count; index++) {
    stats = updateAggregate(
      stats,
      eventScore(algorithmInt(index), algorithmString(index), index),
      index,
    );
  }
  return stats;
}

function summaryLabel(stats) {
  const spread = stats._high - stats._low | 0;
  if (spread > 500) return "analytics-wide";
  if (stats._total < 0) return "analytics-negative";
  if (stats._count >= 6) return "analytics-batch";
  return "analytics-compact";
}

export function renderResult(stats) {
  console.log(summaryLabel(stats));
  return finishAggregate(stats);
}
