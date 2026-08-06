import mitt from "mitt";

const events = mitt();
let score = 0;
let observed = 0;

function record(value) {
  score = (Math.imul(score, 31) + value) | 0;
}

events.on("metric", record);
events.on("*", (type, value) => {
  observed = (observed + value + type.length) | 0;
});

for (let index = 0; index < 180_000; index += 1) {
  events.emit("metric", index % 97);
}

console.log(`events:${score}:${observed}`);
