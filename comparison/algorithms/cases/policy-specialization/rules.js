function clampValue(value) {
  return value < -120 ? -120 : value > 120 ? 120 : value;
}

function modeWeight(mode) {
  return mode === 0 ? 5 : mode === 1 ? -3 : mode === 2 ? 7 : 2;
}

function riskBand(risk) {
  return risk < 0 ? -3 : risk > 80 ? 9 : risk > 40 ? 5 : 2;
}

function baseScore(mode, value, risk) {
  return (clampValue(value) * modeWeight(mode) | 0) + riskBand(risk) | 0;
}

function safeAdjustment(score) {
  return score < 0 ? -score + 7 | 0 : score + 3 | 0;
}

function auditAdjustment(score, enabled) {
  return enabled ? (score * 101 | 0) + 17 | 0 : score;
}

export function evaluateWindow(mode, value, risk) {
  const band = riskBand(risk);
  const adjusted = safeAdjustment(baseScore(mode, value, risk));
  const visible = auditAdjustment(adjusted, false);
  return visible ^ mode + band;
}
