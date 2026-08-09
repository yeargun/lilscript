import { performance } from "node:perf_hooks";

const [library, implementation, mode] = process.argv.slice(2);
if (!global.gc) throw new Error("run with --expose-gc");

async function load() {
  if (library === "nanoid") {
    return implementation === "npm"
      ? import("./node_modules/nanoid/index.browser.js")
      : import("./apps/nanoid/lil/api.js");
  }
  if (library === "mitt") {
    const module = implementation === "npm"
      ? await import("mitt")
      : await import("./apps/mitt/lil/api.js");
    return module.default;
  }
  if (library === "clsx") {
    const module = implementation === "npm"
      ? await import("clsx")
      : await import("./apps/clsx/lil/api.js");
    return module.default;
  }
  if (library === "gl-matrix") {
    globalThis.rand = Math.random;
    return implementation === "npm"
      ? import("gl-matrix/esm/index.js")
      : import("./build/gl-matrix-pm/api.js");
  }
  throw new Error(`unknown library ${library}`);
}

function nanoWork(api, count, retain) {
  let seed = 0;
  const generate = api.customRandom(api.urlAlphabet, 21, (size) => {
    const bytes = new Uint8Array(size);
    for (let index = 0; index < bytes.length; index += 1) {
      bytes[index] = (seed + index * 29) & 255;
    }
    seed = (seed + 17) & 255;
    return bytes;
  });
  const values = retain ? [] : null;
  let checksum = 0;
  for (let index = 0; index < count; index += 1) {
    const value = generate();
    checksum += value.length + value.charCodeAt(0);
    if (values) values.push(value);
  }
  return { checksum, values };
}

function mittWork(createEmitter, count, retain) {
  const emitter = createEmitter();
  let checksum = 0;
  const handler = (value) => {
    checksum += value;
  };
  if (retain) {
    for (let index = 0; index < count; index += 1) {
      emitter.on(`event-${index}`, handler);
      emitter.on(`event-${index}`, handler);
    }
    return { checksum: emitter.all.size, values: emitter };
  }
  emitter.on("event", handler);
  emitter.on("event", handler);
  emitter.on("event", handler);
  emitter.on("*", (_type, value) => {
    checksum += value;
  });
  for (let index = 0; index < count; index += 1) emitter.emit("event", index & 7);
  return { checksum, values: emitter };
}

function clsxWork(clsx, count, retain) {
  const values = retain ? [] : null;
  let checksum = 0;
  for (let index = 0; index < count; index += 1) {
    const value = clsx(
      "base",
      [index & 1 && "odd", ["nested", index]],
      { active: index % 3 === 0, hidden: false },
    );
    checksum += value.length;
    if (values) values.push(value);
  }
  return { checksum, values };
}

function glMatrixWork(api, count, retain) {
  if (retain) {
    const values = [];
    let checksum = 0;
    for (let index = 0; index < count; index += 1) {
      const value = api.vec4.fromValues(index, index + 1, index + 2, index + 3);
      checksum += value.length + value[0];
      values.push(value);
    }
    return { checksum, values };
  }
  const left = api.vec4.fromValues(1, 2, 3, 4);
  const right = api.vec4.fromValues(5, 6, 7, 8);
  const out = api.vec4.create();
  let checksum = 0;
  for (let index = 0; index < count; index += 1) {
    api.vec4.scaleAndAdd(out, left, right, (index & 15) / 16);
    checksum += out[index & 3];
  }
  return { checksum, values: out };
}

function work(api, count, retain) {
  if (library === "nanoid") return nanoWork(api, count, retain);
  if (library === "mitt") return mittWork(api, count, retain);
  if (library === "clsx") return clsxWork(api, count, retain);
  return glMatrixWork(api, count, retain);
}

const api = await load();
const performanceCounts = {
  nanoid: 25_000,
  mitt: 120_000,
  clsx: 75_000,
  "gl-matrix": 300_000,
};
const memoryCounts = {
  nanoid: 12_000,
  mitt: 8_000,
  clsx: 12_000,
  "gl-matrix": 8_000,
};

if (mode === "performance") {
  work(api, Math.floor(performanceCounts[library] / 10), false);
  global.gc();
  const started = performance.now();
  const result = work(api, performanceCounts[library], false);
  console.log(JSON.stringify({ milliseconds: performance.now() - started, checksum: result.checksum }));
} else if (mode === "memory") {
  global.gc();
  const before = process.memoryUsage();
  const result = work(api, memoryCounts[library], true);
  globalThis.__retainedBenchmarkValue = result.values;
  global.gc();
  const after = process.memoryUsage();
  const bytes =
    after.heapUsed - before.heapUsed +
    after.arrayBuffers - before.arrayBuffers;
  console.log(JSON.stringify({ bytes, checksum: result.checksum }));
} else {
  throw new Error(`unknown mode ${mode}`);
}
