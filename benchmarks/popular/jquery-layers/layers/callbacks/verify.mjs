import assert from "node:assert/strict";
import { verify as verifyCore } from "../core-kernel/verify.mjs";

function sameResults(lilCb, jsCb, label) {
  const lil = [];
  const js = [];
  lilCb.add(function (a, b) {
    lil.push([this, a, b]);
  });
  jsCb.add(function (a, b) {
    js.push([this, a, b]);
  });
  return { lil, js, label };
}

export async function verify(lil, js) {
  await verifyCore(lil, js);

  const $l = lil.jQuery;
  const $j = js.jQuery;
  assert.equal(typeof $l.Callbacks, "function");
  assert.equal(typeof $j.Callbacks, "function");

  const plainL = $l.Callbacks();
  const plainJ = $j.Callbacks();
  const seen = sameResults(plainL, plainJ, "plain fire");
  const ctx = { ok: true };
  plainL.fireWith(ctx, [1, 2]);
  plainJ.fireWith(ctx, [1, 2]);
  assert.deepEqual(seen.lil, seen.js, seen.label);
  assert.equal(plainL.fired(), true);
  assert.equal(plainJ.fired(), true);
  assert.equal(plainL.has(), true);
  assert.equal(plainJ.has(), true);

  const fireL = [];
  const fireJ = [];
  const firedL = $l.Callbacks();
  const firedJ = $j.Callbacks();
  firedL.add(function (v) {
    fireL.push([this === firedL, v]);
  });
  firedJ.add(function (v) {
    fireJ.push([this === firedJ, v]);
  });
  firedL.fire(7);
  firedJ.fire(7);
  assert.deepEqual(fireL, fireJ, "fire args");
  assert.deepEqual(fireL, [[true, 7]], "fire this and value");

  const memL = $l.Callbacks("memory");
  const memJ = $j.Callbacks("memory");
  const memSeenL = [];
  const memSeenJ = [];
  memL.add((v) => memSeenL.push(["first", v]));
  memJ.add((v) => memSeenJ.push(["first", v]));
  memL.fire("kept");
  memJ.fire("kept");
  memL.add((v) => memSeenL.push(["late", v]));
  memJ.add((v) => memSeenJ.push(["late", v]));
  assert.deepEqual(memSeenL, memSeenJ, "memory late add");

  const onceL = $l.Callbacks("once");
  const onceJ = $j.Callbacks("once");
  const onceSeenL = [];
  const onceSeenJ = [];
  onceL.add((v) => onceSeenL.push(v));
  onceJ.add((v) => onceSeenJ.push(v));
  onceL.fire(1);
  onceJ.fire(1);
  onceL.fire(2);
  onceJ.fire(2);
  assert.deepEqual(onceSeenL, onceSeenJ, "once");
  assert.equal(onceL.locked(), onceJ.locked(), "once locks");
  assert.equal(onceL.locked(), true, "once stays locked");

  const uniqueL = $l.Callbacks("unique");
  const uniqueJ = $j.Callbacks("unique");
  const uniqueSeenL = [];
  const uniqueSeenJ = [];
  const sharedL = () => uniqueSeenL.push(1);
  const sharedJ = () => uniqueSeenJ.push(1);
  uniqueL.add(sharedL, sharedL);
  uniqueJ.add(sharedJ, sharedJ);
  uniqueL.fire();
  uniqueJ.fire();
  assert.deepEqual(uniqueSeenL, uniqueSeenJ, "unique");

  const stopL = $l.Callbacks("stopOnFalse");
  const stopJ = $j.Callbacks("stopOnFalse");
  const stopSeenL = [];
  const stopSeenJ = [];
  stopL.add(
    () => {
      stopSeenL.push("a");
    },
    () => {
      stopSeenL.push("b");
      return false;
    },
    () => {
      stopSeenL.push("c");
    },
  );
  stopJ.add(
    () => {
      stopSeenJ.push("a");
    },
    () => {
      stopSeenJ.push("b");
      return false;
    },
    () => {
      stopSeenJ.push("c");
    },
  );
  stopL.fire();
  stopJ.fire();
  assert.deepEqual(stopSeenL, stopSeenJ, "stopOnFalse");

  const remL = $l.Callbacks();
  const remJ = $j.Callbacks();
  const remSeenL = [];
  const remSeenJ = [];
  const dropL = () => remSeenL.push("drop");
  const keepL = () => remSeenL.push("keep");
  const dropJ = () => remSeenJ.push("drop");
  const keepJ = () => remSeenJ.push("keep");
  remL.add(dropL, keepL);
  remJ.add(dropJ, keepJ);
  remL.remove(dropL);
  remJ.remove(dropJ);
  remL.fire();
  remJ.fire();
  assert.deepEqual(remSeenL, remSeenJ, "remove");
  assert.equal(remL.has(keepL), remJ.has(keepJ));
  assert.equal(remL.has(dropL), remJ.has(dropJ));

  const emptyL = $l.Callbacks();
  const emptyJ = $j.Callbacks();
  emptyL.add(() => {});
  emptyJ.add(() => {});
  emptyL.empty();
  emptyJ.empty();
  assert.equal(emptyL.has(), emptyJ.has());

  const disL = $l.Callbacks();
  const disJ = $j.Callbacks();
  const disSeenL = [];
  const disSeenJ = [];
  disL.add((v) => disSeenL.push(v));
  disJ.add((v) => disSeenJ.push(v));
  disL.disable();
  disJ.disable();
  disL.add((v) => disSeenL.push(["late", v]));
  disJ.add((v) => disSeenJ.push(["late", v]));
  disL.fire(1);
  disJ.fire(1);
  assert.equal(disL.disabled(), true);
  assert.equal(disJ.disabled(), true);
  assert.deepEqual(disSeenL, disSeenJ, "disable");

  const lockL = $l.Callbacks();
  const lockJ = $j.Callbacks();
  const lockSeenL = [];
  const lockSeenJ = [];
  lockL.add((v) => lockSeenL.push(v));
  lockJ.add((v) => lockSeenJ.push(v));
  lockL.lock();
  lockJ.lock();
  lockL.fire(1);
  lockJ.fire(1);
  assert.equal(lockL.locked(), true);
  assert.equal(lockJ.locked(), true);
  assert.deepEqual(lockSeenL, lockSeenJ, "lock");

  const objL = $l.Callbacks({ once: true, memory: true });
  const objJ = $j.Callbacks({ once: true, memory: true });
  const objSeenL = [];
  const objSeenJ = [];
  objL.fire("early");
  objJ.fire("early");
  objL.add((v) => objSeenL.push(v));
  objJ.add((v) => objSeenJ.push(v));
  objL.fire("again");
  objJ.fire("again");
  assert.deepEqual(objSeenL, objSeenJ, "object once memory");

  const nestL = $l.Callbacks();
  const nestJ = $j.Callbacks();
  const nestSeenL = [];
  const nestSeenJ = [];
  nestL.add([() => nestSeenL.push(1), () => nestSeenL.push(2)]);
  nestJ.add([() => nestSeenJ.push(1), () => nestSeenJ.push(2)]);
  nestL.fire();
  nestJ.fire();
  assert.deepEqual(nestSeenL, nestSeenJ, "nested add");
}
