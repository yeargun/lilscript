import { mix, spring, stagger, wrap } from "motion";

const delay = stagger(0.125, { startDelay: 0.25 });
let position = 0;
let phase = 0;
let schedule = 0;
let springDigest = 0;

for (let index = 0; index < 160_000; index += 1) {
  const lane = index % 8;
  position += mix(-120, 360, lane / 8);
  phase += wrap(0, 360, index * 47);
  schedule += delay(lane, 8) * 8;
}

const generator = spring({
  keyframes: [0, 100],
  stiffness: 170,
  damping: 26,
  mass: 1,
});
for (let time = 0; time <= 1_024; time += 16) {
  springDigest += Math.round(generator.next(time).value * 1_000);
}

console.log(`motion:${position}:${phase}:${schedule}:${springDigest}`);
