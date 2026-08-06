import { cubicBezier, steps } from "@motionone/easing";

const close = (actual, expected, tolerance = 0) =>
  Math.abs(actual - expected) <= tolerance;
let passed = 0;
const check = (condition) => {
  if (condition) passed += 1;
};

const linear = cubicBezier(0, 0, 1, 1);
check(linear(0) === 0);
check(linear(1) === 1);
check(linear(0.5) === 0.5);

const curve = cubicBezier(0.5, 0.1, 0.31, 0.96);
check(curve(0) === 0);
check(close(curve(0.01), 0.002, 0.005));
check(close(curve(0.25), 0.164, 0.005));
check(close(curve(0.75), 0.935, 0.005));
check(close(curve(0.99), 0.999, 0.005));
check(curve(1) === 1);

const stepEnd = steps(4);
for (const [value, expected] of [
  [0, 0], [0.2, 0], [0.249, 0], [0.25, 0.25], [0.49, 0.25],
  [0.5, 0.5], [0.99, 0.75], [1, 0.75],
]) check(stepEnd(value) === expected);

const stepStart = steps(4, "start");
for (const [value, expected] of [
  [0, 0.25], [0.2, 0.25], [0.249, 0.25], [0.25, 0.25], [0.49, 0.5],
  [0.5, 0.5], [0.51, 0.75], [0.99, 1], [1, 1], [2, 1],
]) check(stepStart(value) === expected);

let curveDigest = 0;
let stepDigest = 0;
for (let index = 0; index < 120_000; index += 1) {
  const progress = (index % 101) / 100;
  if (curve(progress) > 0.5) curveDigest += index % 17;
  if (stepEnd(progress) >= 0.5) stepDigest += index % 13;
  if (stepStart(progress) > 0.5) stepDigest += index % 7;
}

console.log(`motion-easing:${passed}:${curveDigest}:${stepDigest}`);
