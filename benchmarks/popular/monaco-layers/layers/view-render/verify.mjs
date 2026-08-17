import assert from "node:assert/strict";

export async function verify(lil) {
  const root = document.createElement("div");
  document.body.appendChild(root);
  const view = lil.mountView(root, "line one\nline two\nline three");
  assert.equal(lil.viewLineCount(view), 3);
  assert.ok(lil.viewScrollHeight(view) > 0);
  assert.equal(lil.viewVisibleStart(view), 1);
  lil.viewLayout(view, 640, 320);
  const html = lil.viewSnapshot(view);
  assert.match(html, /view-line/);
  lil.viewSetTheme(view, "vs-dark");
  assert.match(lil.viewThemeClass(view), /vs-dark/);
}
