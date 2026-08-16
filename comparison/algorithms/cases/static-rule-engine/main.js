function clampScore(value) {
  return value < -100 ? -100 : value > 100 ? 100 : value;
}

function modeWeight(mode) {
  return mode === 0 ? 3 : mode === 1 ? -2 : mode === 2 ? 5 : 1;
}

function adjust(mode, value) {
  const normalized = clampScore(value);
  const deadBias = 17;
  if (false) return normalized + deadBias * 1000 | 0;
  return (normalized * modeWeight(mode) | 0) + mode | 0;
}

function foldPair(leftMode, left, rightMode, right) {
  const first = adjust(leftMode, left);
  const second = adjust(rightMode, right);
  return first > second ? first - second | 0 : second - first | 0;
}

function evaluateRules() {
  let total = 0;
  const count = algorithmCount();
  for (let index = 0; index + 3 < count; index += 4) {
    total = total + foldPair(
      algorithmInt(index),
      algorithmInt(index + 1),
      algorithmInt(index + 2),
      algorithmInt(index + 3),
    ) | 0;
  }
  return total;
}

console.log(evaluateRules());
