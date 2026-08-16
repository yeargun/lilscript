import assert from "node:assert/strict";
import { verify as verifyCallbacks } from "../callbacks/verify.mjs";

function tick() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

export async function verify(lil, js) {
  await verifyCallbacks(lil, js);

  const $l = lil.jQuery;
  const $j = js.jQuery;
  assert.equal(typeof $l.Deferred, "function");
  assert.equal(typeof $j.Deferred, "function");
  assert.equal(typeof $l.when, "function");
  assert.equal(typeof $j.when, "function");
  assert.equal(typeof $l.Deferred.exceptionHook, "function");
  assert.equal(typeof $j.Deferred.exceptionHook, "function");

  const resolvedL = [];
  const resolvedJ = [];
  const dL = $l.Deferred();
  const dJ = $j.Deferred();
  assert.equal(dL.state(), "pending");
  assert.equal(dJ.state(), "pending");
  dL.done((v) => resolvedL.push(["done", v]));
  dJ.done((v) => resolvedJ.push(["done", v]));
  dL.fail((v) => resolvedL.push(["fail", v]));
  dJ.fail((v) => resolvedJ.push(["fail", v]));
  dL.resolve(3);
  dJ.resolve(3);
  assert.equal(dL.state(), "resolved");
  assert.equal(dJ.state(), "resolved");
  assert.deepEqual(resolvedL, resolvedJ, "resolve done");
  assert.deepEqual(resolvedL, [["done", 3]], "resolve value");

  const rejectedL = [];
  const rejectedJ = [];
  $l.Deferred().reject("no").fail((v) => rejectedL.push(v));
  $j.Deferred().reject("no").fail((v) => rejectedJ.push(v));
  assert.deepEqual(rejectedL, rejectedJ, "reject");

  const alwaysL = [];
  const alwaysJ = [];
  $l.Deferred().resolve(1).always((v) => alwaysL.push(v));
  $j.Deferred().resolve(1).always((v) => alwaysJ.push(v));
  assert.deepEqual(alwaysL, alwaysJ, "always");

  const thenL = [];
  const thenJ = [];
  $l.Deferred().resolve(4).then((v) => v + 1).then((v) => thenL.push(v));
  $j.Deferred().resolve(4).then((v) => v + 1).then((v) => thenJ.push(v));
  await tick();
  await tick();
  assert.deepEqual(thenL, thenJ, "then chain");

  const catchL = [];
  const catchJ = [];
  $l.Deferred().reject("x").catch((v) => catchL.push(v));
  $j.Deferred().reject("x").catch((v) => catchJ.push(v));
  await tick();
  assert.deepEqual(catchL, catchJ, "catch");

  const progressL = [];
  const progressJ = [];
  const pL = $l.Deferred();
  const pJ = $j.Deferred();
  pL.progress((v) => progressL.push(v));
  pJ.progress((v) => progressJ.push(v));
  pL.notify("n");
  pJ.notify("n");
  pL.resolve();
  pJ.resolve();
  assert.deepEqual(progressL, progressJ, "notify");

  const promiseL = $l.Deferred().promise();
  const promiseJ = $j.Deferred().promise();
  assert.equal(typeof promiseL.resolve, "undefined");
  assert.equal(typeof promiseJ.resolve, "undefined");
  assert.equal(typeof promiseL.then, "function");
  assert.equal(typeof promiseJ.then, "function");

  const whenEmptyL = [];
  const whenEmptyJ = [];
  $l.when().done((v) => whenEmptyL.push(v ?? "empty"));
  $j.when().done((v) => whenEmptyJ.push(v ?? "empty"));
  assert.deepEqual(whenEmptyL, whenEmptyJ, "when empty");

  const whenOneL = [];
  const whenOneJ = [];
  $l.when(9).done((v) => whenOneL.push(v));
  $j.when(9).done((v) => whenOneJ.push(v));
  assert.deepEqual(whenOneL, whenOneJ, "when one");

  const whenManyL = [];
  const whenManyJ = [];
  $l.when($l.Deferred().resolve("a"), $l.Deferred().resolve("b")).done((a, b) => {
    whenManyL.push([a, b]);
  });
  $j.when($j.Deferred().resolve("a"), $j.Deferred().resolve("b")).done((a, b) => {
    whenManyJ.push([a, b]);
  });
  assert.deepEqual(whenManyL, whenManyJ, "when many");

  const pipeL = [];
  const pipeJ = [];
  $l.Deferred()
    .resolve(2)
    .pipe((v) => v * 3)
    .done((v) => pipeL.push(v));
  $j.Deferred()
    .resolve(2)
    .pipe((v) => v * 3)
    .done((v) => pipeJ.push(v));
  assert.deepEqual(pipeL, pipeJ, "pipe");

  const ctorL = [];
  const ctorJ = [];
  $l.Deferred((d) => d.resolve("init")).done((v) => ctorL.push(v));
  $j.Deferred((d) => d.resolve("init")).done((v) => ctorJ.push(v));
  assert.deepEqual(ctorL, ctorJ, "deferred ctor");
}
