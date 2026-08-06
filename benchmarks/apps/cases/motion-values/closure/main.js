function mixValue(from, to, progress) {
  return from + (to - from) * progress;
}

function wrapValue(min, max, value) {
  const range = max - min;
  return ((((value - min) % range) + range) % range) + min;
}

function staggerDelay(step, start, index) {
  return start + step * index;
}

let position = 0;
let phase = 0;
let schedule = 0;
for (let index = 0; index < 160_000; index += 1) {
  const lane = index % 8;
  position += mixValue(-120, 360, lane / 8);
  phase += wrapValue(0, 360, index * 47);
  schedule += staggerDelay(0.125, 0.25, lane) * 8;
}
console.log(`motion:${position}:${phase}:${schedule}`);
