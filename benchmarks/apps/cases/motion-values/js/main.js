import { mix, stagger, wrap } from "motion";

const delay = stagger(0.125, { startDelay: 0.25 });
let position = 0;
let phase = 0;
let schedule = 0;

for (let index = 0; index < 160_000; index += 1) {
  const lane = index % 8;
  position += mix(-120, 360, lane / 8);
  phase += wrap(0, 360, index * 47);
  schedule += delay(lane, 8) * 8;
}

console.log(`motion:${position}:${phase}:${schedule}`);
