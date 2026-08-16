import { scaleMagnitude, bucketMagnitude } from "./normalize.js";
import { tokenClass } from "./classify.js";

function mixIndex(index) {
  return (index + 3) * 11 | 0;
}

function weightedMagnitude(value, index) {
  return scaleMagnitude(value, index % 4 + 1);
}

function routeWeight(token) {
  return tokenClass(token) * 13 | 0;
}

export function eventScore(value, token, index) {
  return weightedMagnitude(value, index) + routeWeight(token) + mixIndex(index) | 0;
}

export function diagnosticScore(value, token) {
  return (bucketMagnitude(value) * 101 | 0) + tokenClass(token) | 0;
}
