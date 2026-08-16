function isDigitCode(code) {
  return code >= 48 && code <= 57;
}

function isHexLetter(code) {
  return code >= 65 && code <= 70 || code >= 97 && code <= 102;
}

function hexValue(code) {
  if (isDigitCode(code)) return code - 48;
  if (code >= 65 && code <= 70) return code - 55;
  return isHexLetter(code) ? code - 87 : -1;
}

function classifyCode(code) {
  if (isDigitCode(code)) return 1;
  if (isHexLetter(code)) return 2;
  if (code === 58) return 3;
  if (code === 44) return 4;
  if (code === 59) return 5;
  return 0;
}

function advanceState(state, kind) {
  if (kind === 5) return 0;
  if (state === 0 && (kind === 1 || kind === 2)) return 1;
  if (state === 1 && kind === 3) return 2;
  if (state === 2 && (kind === 1 || kind === 2)) return 3;
  if (state === 3 && kind === 4) return 4;
  if (state === 4 && kind === 1) return 5;
  return state;
}

function mixHeader(checksum, digit, index) {
  return (checksum * 17 | 0) ^ digit + (index * 3 | 0);
}

function mixPayload(checksum, digit, state) {
  return (checksum * 31 | 0) + (digit * (state + 1) | 0) | 0;
}

function updateChecksum(checksum, state, kind, code, index) {
  const digit = hexValue(code);
  if (digit < 0) return checksum + kind | 0;
  if (state === 1) return mixHeader(checksum, digit, index);
  if (state === 3 || state === 4) return mixPayload(checksum, digit, state);
  if (state === 5) return checksum + (digit * 7 | 0) | 0;
  return checksum + digit | 0;
}

function finishPacket(state, checksum, fields) {
  return checksum ^ (state * 257 | 0) + (fields * 41 | 0);
}

export function decodePacket(input) {
  let state = 0;
  let checksum = 0;
  let fields = 0;
  for (let index = 0; index < input.length; index++) {
    const code = input.charCodeAt(index);
    if (code === 35) break;
    if (code === 32) continue;
    const kind = classifyCode(code);
    state = advanceState(state, kind);
    checksum = updateChecksum(checksum, state, kind, code, index);
    if (kind === 5) fields++;
  }
  return finishPacket(state, checksum, fields);
}
