import assert from "node:assert/strict";

export async function verify(lil) {
  const ids = lil.allLanguageIds();
  assert.ok(ids.includes("javascript"));
  assert.ok(ids.includes("go"));
  assert.ok(ids.includes("rust"));
  assert.ok(ids.includes("yaml"));
  assert.ok(lil.remainingLanguageIds().length >= 70);
  const tokens = lil.tokenizeRemaining("go", "func main() {\n  return\n}");
  assert.ok(tokens.some((t) => t.type.includes("keyword")));
}
