import { distance, distance2D, mix } from "motion";

let d1 = 0;
let d2 = 0;
let blended = 0;

for (let i = 0; i < 10_000; i += 1) {
  const a = i * 0.017;
  const b = i * 0.031;
  d1 += distance(a, b);
  d2 += distance2D({ x: a, y: b }, { x: b, y: a });
  blended += mix(d1, d2, 0.5);
}

console.log(
  `distance-geometry:${Math.round(d1)}:${Math.round(d2)}:${Math.round(blended)}`,
);
