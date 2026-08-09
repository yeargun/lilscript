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

function createSpring(stiffness, damping, mass, origin, target) {
  const state = { done: false, value: origin };
  const initialVelocity = 0;
  const dampingRatio = damping / (2 * Math.sqrt(stiffness * mass));
  const initialDelta = target - origin;
  const undampedAngularFreq = Math.sqrt(stiffness / mass) / 1000;
  const angularFreq =
    undampedAngularFreq * Math.sqrt(1 - dampingRatio * dampingRatio);
  const amplitude =
    (initialVelocity + dampingRatio * undampedAngularFreq * initialDelta) /
    angularFreq;
  const sinCoeff =
    dampingRatio * undampedAngularFreq * amplitude +
    initialDelta * angularFreq;
  const cosCoeff =
    dampingRatio * undampedAngularFreq * initialDelta -
    amplitude * angularFreq;

  return (time) => {
    const envelope = Math.exp(-dampingRatio * undampedAngularFreq * time);
    const sin = Math.sin(angularFreq * time);
    const cos = Math.cos(angularFreq * time);
    const current =
      target - envelope * (amplitude * sin + initialDelta * cos);
    const currentVelocity =
      1000 * envelope * (sinCoeff * sin + cosCoeff * cos);
    state.done =
      Math.abs(currentVelocity) <= 2 && Math.abs(target - current) <= 0.5;
    state.value = state.done ? target : current;
    return state;
  };
}

let position = 0;
let phase = 0;
let schedule = 0;
let springDigest = 0;
for (let index = 0; index < 160_000; index += 1) {
  const lane = index % 8;
  position += mixValue(-120, 360, lane / 8);
  phase += wrapValue(0, 360, index * 47);
  schedule += staggerDelay(0.125, 0.25, lane) * 8;
}

const spring = createSpring(170, 26, 1, 0, 100);
for (let time = 0; time <= 1024; time += 16) {
  springDigest += Math.round(spring(time).value * 1000);
}

console.log(`motion:${position}:${phase}:${schedule}:${springDigest}`);
