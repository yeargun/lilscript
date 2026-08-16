export function absoluteValue(value) {
  return value < 0 ? -value | 0 : value;
}

export function clampMagnitude(value) {
  return value > 500 ? 500 : value;
}

export function normalizedMagnitude(value) {
  return clampMagnitude(absoluteValue(value));
}

export function scaleMagnitude(value, factor) {
  return normalizedMagnitude(value) * factor | 0;
}

export function bucketMagnitude(value) {
  const normalized = normalizedMagnitude(value);
  return normalized > 300 ? 4 : normalized > 100 ? 3 : normalized > 25 ? 2 : 1;
}
