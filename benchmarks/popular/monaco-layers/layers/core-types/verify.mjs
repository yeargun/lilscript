import assert from "node:assert/strict";

function shape(p) {
  return [p.lineNumber, p.column];
}

function rangeShape(r) {
  return [r.startLineNumber, r.startColumn, r.endLineNumber, r.endColumn];
}

export async function verify(lil, js) {
  const lp = lil.Position(2, 5);
  const jp = new js.Position(2, 5);
  assert.deepEqual(shape(lp), shape(jp));
  assert.equal(lil.positionEquals(lp, lil.Position(2, 5)), true);
  assert.equal(jp.equals(new js.Position(2, 5)), true);

  const lr = lil.Range(1, 10, 1, 3);
  const jr = new js.Range(1, 10, 1, 3);
  assert.deepEqual(rangeShape(lr), rangeShape(jr));
  assert.equal(lil.rangeIsEmpty(lr), jr.isEmpty());

  const ls = lil.Selection(1, 1, 2, 4);
  const jsSel = new js.Selection(1, 1, 2, 4);
  assert.deepEqual(rangeShape(ls), rangeShape(jsSel));
  assert.equal(ls.positionLineNumber, jsSel.positionLineNumber);
  assert.equal(ls.selectionStartColumn, jsSel.selectionStartColumn);
}
