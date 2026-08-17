import assert from "node:assert/strict";

export async function verify(lil) {
  const root = document.createElement("div");
  document.body.appendChild(root);
  const editor = lil.create(root, { value: "hello", language: "javascript", theme: "vs" });
  assert.equal(lil.editorGetValue(editor), "hello");
  lil.editorSetPosition(editor, lil.Position(1, 6));
  lil.editorTrigger(editor, "keyboard", "type", { text: "!" });
  assert.equal(lil.editorGetValue(editor), "hello!");
  lil.editorTrigger(editor, "keyboard", "undo", {});
  assert.equal(lil.editorGetValue(editor), "hello");
  lil.editorTrigger(editor, "keyboard", "editor.action.commentLine", {});
  assert.match(lil.editorGetValue(editor), /\/\//);
  lil.editorTrigger(editor, "keyboard", "editor.action.commentLine", {});
  lil.editorTrigger(editor, "keyboard", "actions.find", {});
  lil.editorTrigger(editor, "keyboard", "editor.action.triggerSuggest", {});
  lil.editorTrigger(editor, "keyboard", "editor.foldAll", {});
  lil.editorTrigger(editor, "keyboard", "editor.unfoldAll", {});
  lil.editorLayout(editor);
  lil.setTheme("vs-dark");
  assert.equal(lil.editorLineCount(editor), 1);
  const diffRoot = document.createElement("div");
  document.body.appendChild(diffRoot);
  const diff = lil.createDiffEditor(diffRoot, { original: "a\nb", modified: "a\nc" });
  const changes = lil.diffLineChanges(diff);
  assert.ok(changes.length >= 1);
  lil.editorDispose(editor);
  lil.diffDispose(diff);
}
