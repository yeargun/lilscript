import assert from "node:assert/strict";

export async function verify(lil) {
  const root = document.createElement("div");
  document.body.appendChild(root);
  const editor = lil.create(root, { value: "function foo() {\n  const bar = 1;\n  return bar;\n}\n", language: "javascript" });
  const found = lil.findInEditor(editor, "bar", false, true, true);
  assert.ok(found.length >= 1);
  const folds = lil.computeIndentFolds(editor);
  assert.ok(folds.length >= 1);
  const hover = lil.hoverAt(editor, lil.editorGetPosition(editor));
  assert.equal(typeof hover, "string");
  const items = lil.suggestAt(editor, lil.editorGetPosition(editor));
  assert.ok(Array.isArray(items));
  assert.equal(lil.expandSnippet("foo$1bar"), "foobar");
  lil.toggleLineComment(editor);
  assert.match(lil.editorGetValue(editor), /\/\//);
  lil.gotoLine(editor, 2);
  assert.equal(lil.editorGetPosition(editor).lineNumber, 2);
  const diff = lil.computeLineDiff("a\nb\n", "a\nc\n");
  assert.ok(diff.length >= 1);
}
