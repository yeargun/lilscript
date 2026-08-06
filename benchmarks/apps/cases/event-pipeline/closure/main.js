class MetricEvents {
  constructor() {
    this.metricHandlers = [];
    this.anyHandlers = [];
  }

  onMetric(handler) {
    this.metricHandlers.push(handler);
  }

  onAny(handler) {
    this.anyHandlers.push(handler);
  }

  emitMetric(value) {
    for (let index = 0; index < this.metricHandlers.length; index += 1) {
      this.metricHandlers[index](value);
    }
    for (let index = 0; index < this.anyHandlers.length; index += 1) {
      this.anyHandlers[index](value);
    }
  }
}

let score = 0;
let observed = 0;
function record(value) {
  score = (Math.imul(score, 31) + value) | 0;
  return score;
}
function recordAny(value) {
  observed = (observed + value + 6) | 0;
  return observed;
}

const events = new MetricEvents();
events.onMetric(record);
events.onAny(recordAny);
for (let index = 0; index < 180_000; index += 1) events.emitMetric(index % 97);
console.log(`events:${score}:${observed}`);
