import clamp from "clamp";
import lerp from "lerp";

let passed = 0;
for (const [value, min, max, expected] of [
  [0, -100, 100, 0], [0, 100, 100, 100], [0, 100, -100, 0],
  [100, 0, 50, 50], [50, 100, 150, 100],
]) if (clamp(value, min, max) === expected) passed += 1;
for (const [from, to, progress, expected] of [
  [0, 1, 0, 0], [-25, 50, 1, 50], [-25, 50, 0, -25],
  [100, 10, 0, 100], [0, 100, 0.5, 50],
]) if (lerp(from, to, progress) === expected) passed += 1;

let position = 0;
let limited = 0;
for (let index = 0; index < 180_000; index += 1) {
  position += lerp(-40, 80, (index % 6) / 6);
  limited += clamp((index % 23) - 11, -5, 7);
}
console.log(`micro-math:${passed}:${position}:${limited}`);
