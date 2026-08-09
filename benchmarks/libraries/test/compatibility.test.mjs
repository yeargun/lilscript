import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { cubicBezier as npmCubicBezier, steps as npmSteps } from "@motionone/easing";
import npmClamp from "clamp";
import npmLerp from "lerp";
import npmStringHash from "string-hash";
import npmLevenshtein from "js-levenshtein";
import npmEmotionHash from "@emotion/hash";
import npmMurmur from "murmurhash-js";
import * as npmRobust from "robust-predicates";

import { clamp as lilClamp } from "../build/ports/clamp.mjs";
import { lerp as lilLerp } from "../build/ports/lerp.mjs";
import { cubicBezier as lilCubicBezier, steps as lilSteps } from "../build/ports/motion-easing.mjs";
import { stringHash as lilStringHash } from "../build/ports/string-hash.mjs";
import { levenshtein as lilLevenshtein } from "../build/ports/js-levenshtein.mjs";
import { emotionHash as lilEmotionHash } from "../build/ports/emotion-hash.mjs";
import {
  murmur as lilMurmur,
  murmur2 as lilMurmur2,
  murmur3 as lilMurmur3,
} from "../build/ports/murmurhash-js.mjs";
import * as lilRobust from "../build/ports/robust-predicates.mjs";

const report = JSON.parse(await readFile(new URL("../build/results.json", import.meta.url), "utf8"));
const compatibility = JSON.parse(await readFile(new URL("../compatibility/libraries.json", import.meta.url), "utf8"));

test("audited exclusions match their pinned published entrypoints", async () => {
  const exclusions = compatibility.auditedButIneligible.filter(
    (entry) => entry.runtimeAudit,
  );
  assert.deepEqual(
    exclusions.map((entry) => entry.package),
    ["nanoid", "yocto-queue"],
  );
  for (const exclusion of exclusions) {
    for (const entrypoint of exclusion.runtimeAudit.auditedEntrypoints) {
      const runtime = await import(entrypoint.specifier);
      const names = Object.keys(runtime).sort();
      const sha256 = createHash("sha256").update(names.join("\n")).digest("hex");
      assert.deepEqual(names, entrypoint.runtimeExportNames, entrypoint.specifier);
      assert.equal(sha256, entrypoint.exportNameSha256, entrypoint.specifier);
      assert.equal(entrypoint.implementedRuntimeExports, 0, entrypoint.specifier);
    }
  }
});

test("only compatibility-gated ports enter the measured result set", () => {
  assert.deepEqual(report.diagnostics.map((result) => result.id), compatibility.ports.map((port) => port.id));
  assert.deepEqual(report.results.map((result) => result.id), report.diagnostics.filter((result) => result.eligible).map((result) => result.id));
  for (const result of report.diagnostics) {
    assert.deepEqual(result.artifacts.map((artifact) => artifact.id), ["vite", "closure", "lilscript"]);
    assert.deepEqual(result.surfaceArtifacts.map((artifact) => artifact.id), ["vite", "closure", "lilscript"]);
    const lilscript = result.artifacts.at(-1);
    assert.equal(lilscript.nativeVerified, true);
    assert.equal(lilscript.cEmitted, true);
    assert.ok(result.translatedAssertions + result.additionalAssertions > 0);
    assert.equal(result.eligible, result.blockers.length === 0);
  }
  for (const result of report.results) {
    const vite = result.surfaceArtifacts[0];
    const closure = result.surfaceArtifacts[1];
    const lilscript = result.surfaceArtifacts[2];
    assert.ok(lilscript.raw <= vite.raw);
    assert.ok(lilscript.raw <= closure.raw);
    assert.ok(lilscript[report.metadata.selectedCodec] <= vite[report.metadata.selectedCodec]);
    assert.ok(lilscript[report.metadata.selectedCodec] <= closure[report.metadata.selectedCodec]);
    assert.ok(result.workload.performance.ratio <= report.metadata.materialRegressionLimit);
    assert.ok(result.workload.retainedMemory.ratio <= report.metadata.materialRegressionLimit);
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

test("js-levenshtein matches installed-package UTF-16 distances", () => {
  const values = [
    "",
    "a",
    "ab",
    "kitten",
    "sitting",
    "A😀Z",
    "café",
    "e\u0301",
    "中文网页",
    "因為我是中國人所以我會說中文",
    "x".repeat(96),
  ];
  for (const left of values) {
    for (const right of values) {
      assert.equal(lilLevenshtein(left, right), npmLevenshtein(left, right), `${left}/${right}`);
    }
  }
});

test("@emotion/hash matches installed-package byte-tail classes", () => {
  const values = [
    "",
    "a",
    "ab",
    "abc",
    "abcd",
    "abcde",
    "something",
    "color: hotpink;",
    "A😀Z",
    "café",
    "中文网页",
    "0123456789abcdef".repeat(32),
  ];
  for (const value of values) assert.equal(lilEmotionHash(value), npmEmotionHash(value), value);
});

test("murmurhash-js matches both algorithms and its default alias", () => {
  const values = ["", "a", "ab", "abc", "abcd", "hello", "LilScript", "0123456789abcdef"];
  for (const value of values) {
    for (const seed of [0, 1, 7, 42, 123456789, 2147483647]) {
      assert.equal(lilMurmur2(value, seed), npmMurmur.murmur2(value, seed));
      assert.equal(lilMurmur3(value, seed), npmMurmur.murmur3(value, seed));
      assert.equal(lilMurmur(value, seed), npmMurmur(value, seed));
    }
  }
});

const robustFixtureRevision = "8bed7fadb4284911e1111876e54a6f8acfa445cd";
const robustFixtures = {
  "orient2d.txt": "46098649d40faec33f7655eefe819e7bf91739abba8e05b868fb4ceeb3260237",
  "orient3d.txt": "41748b47d755700ee83f36ccced66c8f0a6e60e4187eba97a522eb9b06952c74",
  "incircle.txt": "0e6d60367db258d34c1e8f59685996eca9e22db49296447fec96d5c6c3b8b20c",
  "insphere.txt": "94c99bafb73050d33a3c6a4d43cb83eed46459db3bd1a99df9ff9919a5703b58",
};

async function robustFixture(name) {
  const url = `https://raw.githubusercontent.com/mourner/robust-predicates/${robustFixtureRevision}/test/fixtures/${name}`;
  const response = await fetch(url);
  assert.equal(response.ok, true, `${url}: ${response.status}`);
  const contents = await response.text();
  assert.equal(createHash("sha256").update(contents).digest("hex"), robustFixtures[name]);
  return contents.trim().split(/\r?\n/);
}

test("robust-predicates passes all expanded v3.0.3 upstream assertions", async () => {
  const [orient2dLines, orient3dLines, incircleLines, insphereLines] = await Promise.all([
    robustFixture("orient2d.txt"),
    robustFixture("orient3d.txt"),
    robustFixture("incircle.txt"),
    robustFixture("insphere.txt"),
  ]);
  let assertions = 0;
  function translated(condition, message) {
    assertions += 1;
    assert.ok(condition, message);
  }
  function both(name, args, predicate, message) {
    const npm = npmRobust[name](...args);
    const lil = lilRobust[name](...args);
    translated(predicate(npm) && predicate(lil), `${message}: npm=${npm}, lil=${lil}`);
    assert.ok(Object.is(npm, lil), `${name} is not bit-exact for ${message}`);
  }

  both("orient2d", [0, 0, 1, 1, 0, 1], (value) => value < 0, "clockwise");
  both("orient2d", [0, 0, 0, 1, 1, 1], (value) => value > 0, "counterclockwise");
  both("orient2d", [0, 0, 0.5, 0.5, 1, 1], (value) => value === 0, "collinear");
  const r = 0.95;
  const q = 18;
  const p = 16.8;
  const w = 2 ** -43;
  for (let i = 0; i < 128; i += 1) {
    for (let j = 0; j < 128; j += 1) {
      const x = r + w * i / 128;
      const y = r + w * j / 128;
      const npm = npmRobust.orient2d(x, y, q, q, p, p);
      const lil = lilRobust.orient2d(x, y, q, q, p, p);
      translated(Math.sign(npm) === Math.sign(lil), `${x},${y}: ${npm} vs ${lil}`);
      assert.ok(Object.is(npm, lil));
    }
  }
  for (const line of orient2dLines) {
    const [, ax, ay, bx, by, cx, cy, sign] = line.split(" ").map(Number);
    both("orient2d", [ax, ay, bx, by, cx, cy], (value) => Math.sign(value) === -sign, line);
  }
  both("orient2dfast", [0, 0, 1, 1, 0, 1], (value) => value < 0, "clockwise fast");
  both("orient2dfast", [0, 0, 0, 1, 1, 1], (value) => value > 0, "counterclockwise fast");
  both("orient2dfast", [0, 0, 0.5, 0.5, 1, 1], (value) => value === 0, "collinear fast");

  both("incircle", [0, -1, 0, 1, 1, 0, -0.5, 0], (value) => value < 0, "inside");
  both("incircle", [0, -1, 1, 0, 0, 1, -1, 0], (value) => value === 0, "on circle");
  both("incircle", [0, -1, 0, 1, 1, 0, -1.5, 0], (value) => value > 0, "outside");
  both("incircle", [1, 0, -1, 0, 0, 1, 0, -0.9999999999999999], (value) => value < 0, "near inside");
  both("incircle", [1, 0, -1, 0, 0, 1, 0, -1.0000000000000002], (value) => value > 0, "near outside");
  let x = 1e-64;
  for (let i = 0; i < 128; i += 1) {
    both("incircle", [0, x, -x, -x, x, -x, 0, 0], (value) => value > 0, `outside ${x}`);
    both("incircle", [0, x, -x, -x, x, -x, 0, 2 * x], (value) => value < 0, `inside ${x}`);
    both("incircle", [0, x, -x, -x, x, -x, 0, x], (value) => value === 0, `cocircular ${x}`);
    x *= 10;
  }
  for (const line of incircleLines) {
    const [, ax, ay, bx, by, cx, cy, dx, dy, sign] = line.split(" ").map(Number);
    both("incircle", [ax, ay, bx, by, cx, cy, dx, dy], (value) => Math.sign(value) === sign, line);
  }
  both("incirclefast", [0, -1, 0, 1, 1, 0, -0.5, 0], (value) => value < 0, "inside fast");
  both("incirclefast", [0, -1, 0, 1, 1, 0, -1, 0], (value) => value === 0, "on circle fast");
  both("incirclefast", [0, -1, 0, 1, 1, 0, -1.5, 0], (value) => value > 0, "outside fast");

  const orient3dAbove = [0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1];
  const orient3dBelow = [0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, -1];
  const orient3dPlane = [0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0];
  both("orient3d", orient3dAbove, (value) => value > 0, "above");
  both("orient3d", orient3dBelow, (value) => value < 0, "below");
  both("orient3d", orient3dPlane, (value) => value === 0, "coplanar");
  both("orient3d", [...orient3dPlane.slice(0, -1), Number.MIN_VALUE], (value) => value > 0, "near above");
  both("orient3d", [...orient3dPlane.slice(0, -1), -Number.MIN_VALUE], (value) => value < 0, "near below");
  for (const line of orient3dLines) {
    const [, ax, ay, az, bx, by, bz, cx, cy, cz, dx, dy, dz, sign] = line.split(" ").map(Number);
    both("orient3d", [ax, ay, az, bx, by, bz, cx, cy, cz, dx, dy, dz], (value) => Math.sign(value) === sign, line);
    both("orient3d", [dx, dy, dz, bx, by, bz, ax, ay, az, cx, cy, cz], (value) => Math.sign(value) === sign, `${line} symmetry`);
  }
  let randomState = 0x12345678;
  const random = () => {
    randomState ^= randomState << 13;
    randomState ^= randomState >>> 17;
    randomState ^= randomState << 5;
    return (randomState >>> 0) / 0x100000000;
  };
  for (let i = 0; i < 1000; i += 1) {
    const ax = 0.5 + 5e-14 * random();
    const ay = 0.5 + 5e-14 * random();
    const az = 0.5 + 5e-14 * random();
    both("orient3d", [12, 12, 12, 24, 24, 24, 48, 48, 48, ax, ay, az], (value) => value === 0, "degenerate");
    both("orient3d", [24, 24, 24, 48, 48, 48, ax, ay, az, 12, 12, 12], (value) => value === 0, "degenerate permutation");
  }
  both("orient3dfast", orient3dAbove, (value) => value > 0, "above fast");
  both("orient3dfast", orient3dBelow, (value) => value < 0, "below fast");
  both("orient3dfast", orient3dPlane, (value) => value === 0, "coplanar fast");

  const sphereInside = [1, 0, 0, 0, -1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0];
  const sphereOutside = [...sphereInside.slice(0, -1), 2];
  const sphereOn = [...sphereInside.slice(0, -1), -1];
  both("insphere", sphereInside, (value) => value < 0, "inside sphere");
  both("insphere", sphereOutside, (value) => value > 0, "outside sphere");
  both("insphere", sphereOn, (value) => value === 0, "on sphere");
  both("insphere", [...sphereInside.slice(0, -1), -0.9999999999999999], (value) => value < 0, "near inside sphere");
  both("insphere", [...sphereInside.slice(0, -1), -1.0000000000000002], (value) => value > 0, "near outside sphere");
  for (const line of insphereLines) {
    const [, ax, ay, az, bx, by, bz, cx, cy, cz, dx, dy, dz, ex, ey, ez, sign] = line.split(" ").map(Number);
    both("insphere", [ax, ay, az, bx, by, bz, cx, cy, cz, dx, dy, dz, ex, ey, ez], (value) => Math.sign(value) === -sign, line);
  }
  both("inspherefast", sphereInside, (value) => value < 0, "inside sphere fast");
  both("inspherefast", sphereOutside, (value) => value > 0, "outside sphere fast");
  both("inspherefast", sphereOn, (value) => value === 0, "on sphere fast");
  assert.equal(assertions, 23798);
});

test("robust-predicates is bit-exact over an additional dense corpus", () => {
  let assertions = 0;
  function exact(name, args) {
    assertions += 1;
    assert.ok(Object.is(lilRobust[name](...args), npmRobust[name](...args)), `${name}: ${args.join(",")}`);
  }
  const fixed = [
    {
      orient2d: [0, 0, 1, 0, 0, 1],
      orient3d: [0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1],
      incircle: [0, 0, 1, 0, 0, 1, 0.5, 0.5],
      insphere: [0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0.5, 0.5, 0.5],
    },
    {
      orient2d: [-2, 3, 7, -5, 11, 13],
      orient3d: [-2, 3, 5, 7, -5, 2, 11, 13, -3, 17, -19, 23],
      incircle: [-2, 3, 7, -5, 11, 13, 17, -19],
      insphere: [-2, 3, 5, 7, -5, 2, 11, 13, -3, 17, -19, 23, 29, -31, 37],
    },
  ];
  for (const vectors of fixed) {
    for (const name of ["orient2d", "orient2dfast"]) exact(name, vectors.orient2d);
    for (const name of ["orient3d", "orient3dfast"]) exact(name, vectors.orient3d);
    for (const name of ["incircle", "incirclefast"]) exact(name, vectors.incircle);
    for (const name of ["insphere", "inspherefast"]) exact(name, vectors.insphere);
  }
  let state = 0x9e3779b9;
  const random = () => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return (state >>> 0) / 0x80000000 - 1;
  };
  for (let index = 0; index < 30000; index += 1) {
    const scale = index % 3 === 0 ? 1e-40 : index % 3 === 1 ? 1 : 1e40;
    const numbers = Array.from({ length: 15 }, () => random() * scale);
    for (const name of ["orient2d", "orient2dfast"]) exact(name, numbers.slice(0, 6));
    for (const name of ["orient3d", "orient3dfast"]) exact(name, numbers.slice(0, 12));
    for (const name of ["incircle", "incirclefast"]) exact(name, numbers.slice(0, 8));
    for (const name of ["insphere", "inspherefast"]) exact(name, numbers);
  }
  for (let index = 0; index < 20000; index += 1) {
    const offset = (index - 10000) * Number.EPSILON;
    const orient2d = [0.95 + offset, 0.95 - offset, 18, 18, 16.8, 16.8];
    const orient3d = [12, 12, 12, 24, 24, 24, 48, 48, 48, 0.5 + offset, 0.5 - offset, 0.5 + offset];
    exact("orient2d", orient2d);
    exact("orient2dfast", orient2d);
    exact("orient3d", orient3d);
    exact("orient3dfast", orient3d);
  }
  assert.equal(assertions, 320016);
});

test("Closure inputs contain the installed package implementations", async () => {
  const markers = new Map([
    ["motion-easing", ["node_modules/@motionone/easing", "node_modules/@motionone/utils"]],
    ["micro-math", ["node_modules/clamp/index.js", "node_modules/lerp/index.js"]],
    ["string-hash", ["node_modules/string-hash/index.js"]],
    ["js-levenshtein", ["node_modules/js-levenshtein/index.js"]],
    ["emotion-hash", ["node_modules/@emotion/hash"]],
    ["murmurhash-js", ["node_modules/murmurhash-js/murmurhash2_gc.js", "node_modules/murmurhash-js/murmurhash3_gc.js"]],
    ["robust-predicates", ["node_modules/robust-predicates/esm/"]],
  ]);
  for (const [id, expectedMarkers] of markers) {
    const input = await readFile(new URL(`../build/${id}/closure-input.js`, import.meta.url), "utf8");
    for (const marker of expectedMarkers) assert.match(input, new RegExp(marker.replaceAll("/", "\\/")));
  }
});

test("ineligible dynamic packages are not presented as complete", () => {
  const rejected = new Set(report.auditedButIneligible.map((item) => item.package));
  for (const name of ["motion", "nanoid", "yocto-queue"]) {
    assert.equal(rejected.has(name), true);
  }
  assert.equal(rejected.has("clsx"), false);
  assert.equal(rejected.has("mitt"), false);
  assert.equal(rejected.has("robust-predicates"), false);
});
