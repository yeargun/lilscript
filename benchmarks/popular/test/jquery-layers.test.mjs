import assert from "node:assert/strict";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { extractLayer } from "../jquery-layers/extract-upstream.mjs";
import { layers, planned } from "../jquery-layers/catalog.mjs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const upstreamRoot = join(labRoot, "../node_modules/jquery/src");

test("utilities extract is the real jquery src, not a rewrite", () => {
  const source = extractLayer("utilities", upstreamRoot);
  assert.match(source, /function isFunction\(/);
  assert.match(source, /typeof obj === "function"/);
  assert.match(source, /function camelCase\(/);
  assert.match(source, /rmsPrefix/);
  assert.match(source, /function toType\(/);
  assert.match(source, /class2type\[ toString\.call\( obj \) \]/);
  assert.match(source, /export \{ isFunction, isWindow, toType, camelCase, nodeName, stripAndCollapse \}/);
  assert.equal(layers[0].id, planned[0]);
});

test("core-kernel extract is the real jquery src, not a rewrite", () => {
  const source = extractLayer("core-kernel", upstreamRoot);
  assert.match(source, /jQuery\.extend = jQuery\.fn\.extend/);
  assert.match(source, /function isArrayLike/);
  assert.match(source, /jQuery\.fn = jQuery\.prototype/);
  assert.match(source, /return new jQuery\.fn\.init/);
  assert.match(source, /function DOMEval/);
  assert.match(source, /function toType\(/);
  assert.match(source, /export \{ jQuery \}/);
  assert.equal(layers[1].id, planned[1]);
  assert.deepEqual(layers[1].dependsOn, ["utilities"]);
});

test("callbacks extract is the real jquery src, not a rewrite", () => {
  const source = extractLayer("callbacks", upstreamRoot);
  assert.match(source, /jQuery\.Callbacks = function/);
  assert.match(source, /function createOptions/);
  assert.match(source, /options\.stopOnFalse/);
  assert.match(source, /rnothtmlwhite/);
  assert.match(source, /jQuery\.extend = jQuery\.fn\.extend/);
  assert.match(source, /export \{ jQuery \}/);
  assert.equal(layers[2].id, planned[2]);
  assert.deepEqual(layers[2].dependsOn, ["core-kernel"]);
});

test("deferred extract is the real jquery src, not a rewrite", () => {
  const source = extractLayer("deferred", upstreamRoot);
  assert.match(source, /Deferred:\s*function/);
  assert.match(source, /function adoptValue/);
  assert.match(source, /when:\s*function/);
  assert.match(source, /jQuery\.Deferred\.exceptionHook/);
  assert.match(source, /jQuery\.Callbacks = function/);
  assert.match(source, /export \{ jQuery \}/);
  assert.equal(layers[3].id, planned[3]);
  assert.deepEqual(layers[3].dependsOn, ["callbacks"]);
});
