import { performance } from "node:perf_hooks";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const [library, implementation, mode, moduleRoot] = process.argv.slice(2);
if (!global.gc) throw new Error("run with --expose-gc");

function lilModule(file) {
  return moduleRoot
    ? pathToFileURL(resolve(moduleRoot, file)).href
    : `./build/ports/${file}`;
}

const lilModules = {
  "motion-easing": lilModule("motion-easing.mjs"),
  "micro-math": null,
  "string-hash": lilModule("string-hash.mjs"),
  "js-levenshtein": lilModule("js-levenshtein.mjs"),
  "emotion-hash": lilModule("emotion-hash.mjs"),
  "murmurhash-js": lilModule("murmurhash-js.mjs"),
  "robust-predicates": lilModule("robust-predicates.mjs"),
};

async function load() {
  if (implementation === "lilscript") {
    if (library === "micro-math") {
      const [clamp, lerp] = await Promise.all([
        import(lilModule("clamp.mjs")),
        import(lilModule("lerp.mjs")),
      ]);
      return { clamp: clamp.clamp, lerp: lerp.lerp };
    }
    const module = await import(lilModules[library]);
    if (library === "motion-easing") return module;
    if (library === "string-hash") return module.stringHash;
    if (library === "js-levenshtein") return module.levenshtein;
    if (library === "emotion-hash") return module.emotionHash;
    return module;
  }
  if (library === "motion-easing") return import("@motionone/easing");
  if (library === "micro-math") {
    const [clamp, lerp] = await Promise.all([import("clamp"), import("lerp")]);
    return { clamp: clamp.default, lerp: lerp.default };
  }
  if (library === "string-hash") return (await import("string-hash")).default;
  if (library === "js-levenshtein") return (await import("js-levenshtein")).default;
  if (library === "emotion-hash") return (await import("@emotion/hash")).default;
  if (library === "murmurhash-js") return (await import("murmurhash-js")).default;
  if (library === "robust-predicates") return import("robust-predicates");
  throw new Error(`unknown library ${library}`);
}

const textValues = [
  "",
  "a",
  "ab",
  "abc",
  "kitten",
  "sitting",
  "color:hotpink;display:grid",
  "A😀Z",
  "café",
  "中文网页",
  "0123456789abcdef".repeat(8),
];

function motionWork(api, count, retain) {
  const curves = [
    [0, 0, 1, 1],
    [0.5, 0.1, 0.31, 0.96],
    [0.42, 0, 1, 1],
    [0, 0, 0.58, 1],
    [0.33, 1.53, 0.69, 0.99],
  ];
  const easings = curves.map((curve) => api.cubicBezier(...curve));
  const stepped = [api.steps(3, "end"), api.steps(7, "start")];
  let checksum = 0;
  const values = retain ? [] : null;
  for (let index = 0; index < count; index += 1) {
    if (retain) {
      const curve = curves[index % curves.length];
      const easing = api.cubicBezier(...curve);
      const step = api.steps(index % 11 + 1, index & 1 ? "start" : "end");
      checksum += easing(0.375) + step(0.375);
      values.push(easing, step);
    } else {
      const progress = (index & 1023) / 1023;
      checksum += easings[index % easings.length](progress);
      checksum += stepped[index & 1](progress);
    }
  }
  return { checksum, values: values ?? easings };
}

function microMathWork(api, count, retain) {
  let checksum = 0;
  const values = retain ? new Array(count) : null;
  for (let index = 0; index < count; index += 1) {
    const value = (index & 255) - 96.5;
    const result = api.lerp(api.clamp(value, -12.5, 72.25), 19.75, (index & 31) / 31);
    checksum += result;
    if (values) values[index] = result;
  }
  return { checksum, values };
}

function stringHashWork(hash, count, retain) {
  let checksum = 0;
  const values = retain ? new Array(count) : null;
  for (let index = 0; index < count; index += 1) {
    const result = hash(`${textValues[index % textValues.length]}:${index & 127}`);
    checksum += result;
    if (values) values[index] = result;
  }
  return { checksum, values };
}

function levenshteinWork(distance, count, retain) {
  let checksum = 0;
  const values = retain ? new Array(count) : null;
  for (let index = 0; index < count; index += 1) {
    const left = textValues[index % textValues.length];
    const right = textValues[(index * 7 + 3) % textValues.length];
    const result = distance(left, right);
    checksum += result;
    if (values) values[index] = result;
  }
  return { checksum, values };
}

function emotionWork(hash, count, retain) {
  let checksum = 0;
  const values = retain ? new Array(count) : null;
  for (let index = 0; index < count; index += 1) {
    const result = hash(`${textValues[index % textValues.length]}:${index & 255}`);
    checksum += result.length + result.charCodeAt(0);
    if (values) values[index] = result;
  }
  return { checksum, values };
}

function murmurWork(api, count, retain) {
  let checksum = 0;
  const values = retain ? new Array(count * 2) : null;
  for (let index = 0; index < count; index += 1) {
    const value = `${textValues[index % textValues.length]}:${index & 63}`;
    const seed = (index * 2654435761) | 0;
    const result2 = api.murmur2(value, seed);
    const result3 = api.murmur3(value, seed);
    checksum += result2 + result3;
    if (values) {
      values[index * 2] = result2;
      values[index * 2 + 1] = result3;
    }
  }
  return { checksum, values };
}

function robustPredicatesWork(api, count, retain) {
  let checksum = 0;
  const values = retain ? new Array(count * 8) : null;
  for (let index = 0; index < count; index += 1) {
    const phase = (index & 1023) / 1024;
    const scale = index % 3 === 0 ? 1e-40 : index % 3 === 1 ? 1 : 1e40;
    const ax = (phase - 0.25) * scale;
    const ay = (((index * 17) & 1023) / 1024 - 0.5) * scale;
    const az = (((index * 29) & 1023) / 1024 - 0.5) * scale;
    const bx = (0.75 - phase * 0.25) * scale;
    const by = (0.125 + phase) * scale;
    const bz = (0.625 - phase * 0.5) * scale;
    const cx = (-0.375 + phase * 0.5) * scale;
    const cy = (0.875 - phase * 0.25) * scale;
    const cz = (-0.75 + phase) * scale;
    const dx = (0.25 + phase * 0.125) * scale;
    const dy = (-0.625 + phase * 0.75) * scale;
    const dz = (0.375 - phase * 0.25) * scale;
    const ex = (-0.125 + phase * 0.375) * scale;
    const ey = (0.5 - phase * 0.125) * scale;
    const ez = (-0.25 + phase * 0.625) * scale;
    const result = [
      api.orient2d(ax, ay, bx, by, cx, cy),
      api.orient2dfast(ax, ay, bx, by, cx, cy),
      api.orient3d(ax, ay, az, bx, by, bz, cx, cy, cz, dx, dy, dz),
      api.orient3dfast(ax, ay, az, bx, by, bz, cx, cy, cz, dx, dy, dz),
      api.incircle(ax, ay, bx, by, cx, cy, dx, dy),
      api.incirclefast(ax, ay, bx, by, cx, cy, dx, dy),
      api.insphere(ax, ay, az, bx, by, bz, cx, cy, cz, dx, dy, dz, ex, ey, ez),
      api.inspherefast(ax, ay, az, bx, by, bz, cx, cy, cz, dx, dy, dz, ex, ey, ez),
    ];
    for (let resultIndex = 0; resultIndex < result.length; resultIndex += 1) {
      checksum += result[resultIndex];
      if (values) values[index * 8 + resultIndex] = result[resultIndex];
    }
  }
  return { checksum, values };
}

function work(api, count, retain) {
  if (library === "motion-easing") return motionWork(api, count, retain);
  if (library === "micro-math") return microMathWork(api, count, retain);
  if (library === "string-hash") return stringHashWork(api, count, retain);
  if (library === "js-levenshtein") return levenshteinWork(api, count, retain);
  if (library === "emotion-hash") return emotionWork(api, count, retain);
  if (library === "robust-predicates") return robustPredicatesWork(api, count, retain);
  return murmurWork(api, count, retain);
}

const performanceCounts = {
  "motion-easing": 120_000,
  "micro-math": 1_000_000,
  "string-hash": 150_000,
  "js-levenshtein": 300_000,
  "emotion-hash": 150_000,
  "murmurhash-js": 120_000,
  "robust-predicates": 12_000,
};
const memoryCounts = {
  "motion-easing": 12_000,
  "micro-math": 40_000,
  "string-hash": 40_000,
  "js-levenshtein": 40_000,
  "emotion-hash": 20_000,
  "murmurhash-js": 20_000,
  "robust-predicates": 4_000,
};

const api = await load();
if (mode === "contract") {
  const result = work(api, 64, false);
  process.stdout.write(JSON.stringify({ checksum: result.checksum }));
} else if (mode === "performance") {
  work(api, Math.floor(performanceCounts[library] / 10), false);
  global.gc();
  const started = performance.now();
  const result = work(api, performanceCounts[library], false);
  process.stdout.write(JSON.stringify({
    milliseconds: performance.now() - started,
    checksum: result.checksum,
  }));
} else if (mode === "memory") {
  // Reach a stable optimized tier before taking the retained-allocation
  // baseline. A tenth-sized warmup leaves larger generated functions on the
  // JIT threshold, so compiler-code retention is nondeterministically charged
  // to the measured library result instead of the warmup phase.
  work(api, memoryCounts[library], false);
  global.gc();
  const before = process.memoryUsage();
  const result = work(api, memoryCounts[library], true);
  globalThis.__retainedLibraryBenchmark = result.values;
  global.gc();
  const after = process.memoryUsage();
  process.stdout.write(JSON.stringify({
    bytes: after.heapUsed - before.heapUsed + after.arrayBuffers - before.arrayBuffers,
    checksum: result.checksum,
  }));
} else {
  throw new Error(`unknown mode ${mode}`);
}
