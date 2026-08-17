import assert from "node:assert/strict";

export async function verify(lil) {
  const root = document.createElement("div");
  document.body.appendChild(root);
  const view = lil.mountCommands(root, "abc");
  lil.selectAll(view);
  lil.typeText(view, "!");
  assert.equal(lil.commandValue(view), "!");
  lil.typeText(view, "hi");
  assert.equal(lil.commandValue(view), "!hi");
  lil.deleteLeft(view);
  assert.equal(lil.commandValue(view), "!h");
  lil.undoEdit(view);
  assert.equal(lil.commandValue(view), "!hi");
  lil.triggerCommand(view, "selectAll", "");
  lil.triggerCommand(view, "type", "z");
  assert.equal(lil.commandValue(view), "z");
}
