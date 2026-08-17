import assert from "node:assert/strict";

export async function verify(lil) {
  lil.registerPopularLanguages();
  const js = lil.tokenize("javascript", "const x = 1; // c\nfunction f() { return x; }");
  assert.ok(js.some((t) => t.type.includes("keyword")));
  assert.ok(js.some((t) => t.type.includes("comment") || t.type.includes("number")));
  const json = lil.tokenize("json", '{ "a": true }');
  assert.ok(json.some((t) => t.type.includes("string") || t.type.includes("keyword")));
  const py = lil.tokenize("python", "def foo():\n  return 1");
  assert.ok(py.some((t) => t.type.includes("keyword")));
  const ids = lil.languageIds();
  for (const id of ["javascript", "typescript", "json", "python", "html", "css", "markdown"]) {
    assert.ok(ids.includes(id), id);
  }
}
