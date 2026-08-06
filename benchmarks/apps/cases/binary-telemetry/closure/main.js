function decodeFrame(seed, sharedBytes) {
  const bytes = new Uint8Array(new ArrayBuffer(96));
  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] =
      ((((seed + Math.imul(index, 17)) | 0) + (index % 7)) | 0) % 256;
  }
  const payload = bytes.subarray(4, 92);
  let checksum = 0;
  let peaks = 0;
  for (let index = 0; index < payload.length; index += 1) {
    const value = payload[index];
    checksum =
      ((checksum + Math.imul(value, (index + 3) | 0)) | 0) % 1_000_000_007;
    if (value > 239) peaks += 1;
  }
  sharedBytes[seed % sharedBytes.length] = checksum % 256;
  return ((checksum + Math.imul(peaks, 257)) | 0) % 1_000_000_007;
}

const shared = new SharedArrayBuffer(32);
const sharedBytes = new Uint8Array(shared);
let digest = 0;
for (let round = 0; round < 20_000; round += 1) {
  digest = ((digest + decodeFrame(round % 251, sharedBytes)) | 0) % 1_000_000_007;
}
for (let index = 0; index < sharedBytes.length; index += 1) {
  digest =
    ((digest + Math.imul(sharedBytes[index], (index + 1) | 0)) | 0) %
    1_000_000_007;
}
console.log(`binary:${digest}:${shared.byteLength}`);
