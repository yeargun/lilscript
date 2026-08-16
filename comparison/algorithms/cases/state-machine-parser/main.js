function isDigit(code) {
  return code >= 48 && code <= 57;
}

function isLetter(code) {
  return code >= 65 && code <= 90 || code >= 97 && code <= 122;
}

function characterKind(code) {
  if (isDigit(code)) return 1;
  if (isLetter(code)) return 2;
  if (code === 58) return 3;
  if (code === 59) return 4;
  return 0;
}

function nextState(state, kind) {
  if (kind === 4) return 0;
  if (state === 0 && kind === 2) return 1;
  if (state === 1 && kind === 3) return 2;
  if (state === 2 && kind === 1) return 3;
  return state;
}

function contribution(state, kind, code, index) {
  if (state === 1) return code + index | 0;
  if (state === 2) return kind * 17 | 0;
  if (state === 3) return (code - 48 | 0) * (index + 1) | 0;
  return kind;
}

function finishState(state, checksum) {
  return (checksum * 5 | 0) + (state * 97 | 0) | 0;
}

function parseStateMachine(input) {
  let state = 0;
  let checksum = 0;
  for (let index = 0; index < input.length; index++) {
    const code = input.charCodeAt(index);
    if (code === 35) break;
    if (code === 32) continue;
    const kind = characterKind(code);
    state = nextState(state, kind);
    checksum = checksum + contribution(state, kind, code, index) | 0;
  }
  return finishState(state, checksum);
}

console.log(parseStateMachine(algorithmString(0)));
