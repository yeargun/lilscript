// Labels describe exact deterministic bytes for an eligible matched boundary.
// Runtime parity and robustness across programs are separate evidence.
export const SIZE_VERDICT_POLICY = Object.freeze({
  version: "exact-bytes-v1",
  smaller: "A negative candidate-minus-reference byte delta, including one byte.",
  equal: "Identical byte counts; the artifacts need not be byte-identical.",
  larger: "A positive candidate-minus-reference byte delta.",
  ineligible: "Missing measurement or unmatched/unqualified behavior contract.",
  robustness: "Separate source/name perturbations and held-out results; no universal byte threshold.",
  cost: "Report added judgments and compilation CPU separately; a small saving does not imply a useful default.",
  historical: "Previously published policies/verdicts retain their original version and thresholds.",
});

export function sizeVerdict(candidate, reference, eligible = true) {
  for (const value of [candidate, reference]) {
    if (value !== null && (!Number.isSafeInteger(value) || value < 0))
      throw new TypeError("sizes must be nonnegative exact integers or null");
  }
  const delta = candidate === null || reference === null ? null : candidate - reference;
  return {policy: SIZE_VERDICT_POLICY.version, candidate, reference, delta,
    fraction: delta === null || reference === 0 ? null : delta / reference,
    label: !eligible || delta === null ? "ineligible" : delta < 0 ? "smaller" : delta > 0 ? "larger" : "equal"};
}
