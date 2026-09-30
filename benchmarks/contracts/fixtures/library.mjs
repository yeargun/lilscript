export function transform(value) { return ((value * 2 | 0) + Math.imul(value & 3, 3)) | 0; }
export function unused(value) { return String(value).repeat(20); }
