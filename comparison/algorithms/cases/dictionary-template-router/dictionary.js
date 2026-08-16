const keywordWeights = {
  "render": 31,
  "hydrate": 37,
  "prefetch": 41,
  "invalidate": 43,
  "commit": 47,
};

function keywordWeight(value) {
  return Object.hasOwn(keywordWeights, value) ? keywordWeights[value] : 5;
}

function prefixWeight(value) {
  return value.startsWith("cache:") ? 17
    : value.startsWith("view:") ? 19
    : value.startsWith("data:") ? 23
    : 3;
}

function suffixWeight(value) {
  return value.endsWith(":hot") ? 13
    : value.endsWith(":cold") ? -7
    : value.endsWith(":idle") ? 2
    : 1;
}

function shapeWeight(value) {
  return value.length === 0 ? 0 : value.charCodeAt(0) % 13;
}

function lengthWeight(value, index) {
  return value.length * (index + 3) | 0;
}

function tokenClass(value) {
  return (prefixWeight(value) * 3 | 0) + (suffixWeight(value) * 5 | 0) | 0;
}

export function tokenScore(value, index) {
  return keywordWeight(value) + tokenClass(value) + shapeWeight(value) + lengthWeight(value, index) | 0;
}

export function labelFor(total) {
  const group = (total % 5 + 5) % 5;
  return [
    "template-route-stable",
    "template-route-warm",
    "template-route-cold",
    "template-route-deferred",
    "template-route-retry",
  ][group];
}

export function renderDigest(total, count) {
  return (total * 7 | 0) + (count * 29 | 0) | 0;
}
