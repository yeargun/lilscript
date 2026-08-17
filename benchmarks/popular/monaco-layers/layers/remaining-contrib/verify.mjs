import assert from "node:assert/strict";

export async function verify(lil) {
  const ids = lil.remainingContribIds();
  assert.ok(ids.includes("format"));
  assert.ok(ids.includes("rename"));
  assert.ok(ids.includes("diffEditor"));
  assert.equal(lil.inlayHintEnabled(), true);
  assert.equal(lil.unicodeHighlightEnabled(), true);
  const hints = lil.parameterHints(null);
  assert.ok(hints.length >= 1);
}
