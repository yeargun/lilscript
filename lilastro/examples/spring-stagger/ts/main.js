import { mix, spring, stagger } from "motion";

const delay = stagger(0.08, { startDelay: 0.1, from: "first" });
let schedule = 0;
let springDigest = 0;
let blend = 0;

for (let i = 0; i < 48; i += 1) {
  schedule += delay(i, 48) * 1000;
  blend += mix(0, 1, i / 47);
}

const gen = spring({
  keyframes: [0, 1],
  stiffness: 300,
  damping: 20,
  mass: 1,
});
for (let t = 0; t <= 800; t += 10) {
  springDigest += Math.round(gen.next(t).value * 10_000);
}

console.log(`spring-stagger:${schedule.toFixed(2)}:${blend.toFixed(4)}:${springDigest}`);
