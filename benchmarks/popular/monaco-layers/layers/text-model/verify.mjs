import assert from "node:assert/strict";

export async function verify(lil) {
  const model = lil.createModel("hello\nworld");
  assert.equal(lil.modelGetValue(model), "hello\nworld");
  assert.equal(lil.modelLineCount(model), 2);
  lil.modelApplyEdits(model, [lil.editOp(1, 6, 1, 6, "!")], true);
  assert.equal(lil.modelGetValue(model), "hello!\nworld");
  assert.equal(lil.modelUndo(model), true);
  assert.equal(lil.modelGetValue(model), "hello\nworld");
  assert.equal(lil.modelRedo(model), true);
  assert.equal(lil.modelGetValue(model), "hello!\nworld");

  const ids = lil.modelDeltaDecorations(model, [], [lil.deco(1, 1, 1, 6, "x")]);
  assert.equal(ids.length, 1);
  const found = lil.modelDecorationsInRange(model, lil.createRange(1, 1, 1, 6));
  assert.equal(found.length, 1);

  const matches = lil.modelFindMatches(model, "hello", false, false, false, 10);
  assert.equal(matches.length, 1);
  assert.equal(matches[0].range.startColumn, 1);
  lil.modelSetValue(model, "ab ab ab");
  assert.equal(lil.modelFindMatches(model, "ab", false, false, true, 10).length, 3);
}
