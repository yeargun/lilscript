import { weight } from "./math.js";

export function checksum(values) {
  let total = 0;
  for (let index = 0; index < values.length; index = (index + 1) | 0) {
    total = (total + weight(values[index], index)) | 0;
  }
  return total;
}

export function unusedStats(values) {
  return Math.imul(values.length, 1000);
}
