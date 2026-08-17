import assert from "node:assert/strict";

export async function verify(lil) {
  assert.equal(lil.languageServicesWithoutTsc(), true);
  const css = lil.cssCompletions("mar");
  assert.ok(css.some((item) => item.label === "margin"));
  const html = lil.htmlCompletions("di");
  assert.ok(html.some((item) => item.label === "div"));
}
