import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { cubicBezier as npmCubicBezier, steps as npmSteps } from "@motionone/easing";
import npmClamp from "clamp";
import npmLerp from "lerp";
import npmStringHash from "string-hash";

import { clamp as lilClamp } from "../build/ports/clamp.mjs";
import { lerp as lilLerp } from "../build/ports/lerp.mjs";
import { cubicBezier as lilCubicBezier, steps as lilSteps } from "../build/ports/motion-easing.mjs";
import { stringHash as lilStringHash } from "../build/ports/string-hash.mjs";

const report = JSON.parse(await readFile(new URL("../build/results.json", import.meta.url), "utf8"));
const compatibility = JSON.parse(await readFile(new URL("../compatibility/libraries.json", import.meta.url), "utf8"));

test("only compatibility-gated ports enter the measured result set", () => {
  assert.deepEqual(report.results.map((result) => result.id), compatibility.ports.map((port) => port.id));
  for (const result of report.results) {
    assert.deepEqual(result.artifacts.map((artifact) => artifact.id), ["vite", "closure", "lilscript"]);
    const lilscript = result.artifacts.at(-1);
    assert.equal(lilscript.nativeVerified, true);
    assert.equal(lilscript.cEmitted, true);
    assert.ok(result.translatedAssertions > 0);
  }
});

test("the complete Motion easing entrypoint matches over a dense curve grid", () => {
  const curves = [
    [0, 0, 1, 1],
    [0.5, 0.1, 0.31, 0.96],
    [0.42, 0, 1, 1],
    [0, 0, 0.58, 1],
    [0.33, 1.53, 0.69, 0.99],
  ];
  for (const definition of curves) {
    const npm = npmCubicBezier(...definition);
    const lil = lilCubicBezier(...definition);
    for (let index = 0; index <= 1000; index += 1) {
      const progress = index / 1000;
      assert.ok(Math.abs(npm(progress) - lil(progress)) <= Number.EPSILON);
    }
  }
});

test("Motion steps matches directions, boundaries, and signed zero", () => {
  for (const count of [1, 2, 3, 4, 7, 12]) {
    for (const direction of ["end", "start", "unexpected"]) {
      const npm = npmSteps(count, direction);
      const lil = lilSteps(count, direction);
      for (const progress of [-2, -0, 0, 0.001, 0.249, 0.5, 0.999, 1, 2]) {
        assert.ok(Object.is(npm(progress), lil(progress)), `${count}/${direction}/${progress}`);
      }
    }
  }
});

test("clamp and lerp match documented numeric domains", () => {
  const numbers = [-100, -7.5, -0, 0, 0.25, 1, 12.5, 100];
  for (const value of numbers) {
    for (const first of numbers) {
      for (const second of numbers) {
        assert.ok(Object.is(npmClamp(value, first, second), lilClamp(value, first, second)));
      }
    }
  }
  for (const from of numbers) {
    for (const to of numbers) {
      for (const progress of [-1, -0, 0, 0.25, 0.5, 1, 2]) {
        assert.ok(Object.is(npmLerp(from, to, progress), lilLerp(from, to, progress)));
      }
    }
  }
});

test("string-hash matches UTF-16 input classes", () => {
  const values = [
    "",
    "Mary had a little lamb.",
    "Hello, world!",
    "A😀Z",
    "café",
    "e\u0301",
    "中文网页",
    "\0inside",
    "🙂🙃🙂🙃",
    "LilScript".repeat(128),
  ];
  for (const value of values) assert.equal(lilStringHash(value), npmStringHash(value), value);
});

test("Closure inputs contain the installed package implementations", async () => {
  const markers = new Map([
    ["motion-easing", ["node_modules/@motionone/easing", "node_modules/@motionone/utils"]],
    ["micro-math", ["node_modules/clamp/index.js", "node_modules/lerp/index.js"]],
    ["string-hash", ["node_modules/string-hash/index.js"]],
  ]);
  for (const [id, expectedMarkers] of markers) {
    const input = await readFile(new URL(`../build/${id}/closure-input.js`, import.meta.url), "utf8");
    for (const marker of expectedMarkers) assert.match(input, new RegExp(marker.replaceAll("/", "\\/")));
  }
});

test("ineligible dynamic packages are not presented as complete", () => {
  const rejected = new Set(report.auditedButIneligible.map((item) => item.package));
  for (const name of ["motion", "mitt", "nanoid", "clsx", "yocto-queue", "@emotion/hash"]) {
    assert.equal(rejected.has(name), true);
  }
});
