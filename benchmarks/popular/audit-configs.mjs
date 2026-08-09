import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";
import { performance } from "node:perf_hooks";

const root = dirname(fileURLToPath(import.meta.url));
const repo = resolve(root, "../..");
const compiler = join(repo, "target/release/lilscript");
const outputRoot = join(root, "build/config-audit");
mkdirSync(outputRoot, { recursive: true });

const variants = [
  {
    name: "fast",
    priority: "performance-first",
    level: 0,
    search: "off",
    limit: 1,
    beam: 1,
  },
  {
    name: "balanced",
    priority: "balanced",
    level: 8,
    search: "production",
    limit: 64,
    beam: 4,
  },
  {
    name: "size",
    priority: "size-first",
    level: 12,
    search: "production",
    limit: 256,
    beam: 6,
  },
  {
    name: "maximum",
    priority: "size-first",
    level: 15,
    search: "production",
    limit: 1536,
    beam: 12,
  },
];

const libraries = [
  {
    name: "nanoid",
    input: join(root, "ports/nanoid/index.lil"),
    functionSpelling: "arrow",
    properties: true,
    verify(module) {
      assert.deepEqual(
        Object.keys(module).sort(),
        ["customAlphabet", "customRandom", "nanoid", "random", "urlAlphabet"],
      );
      assert.equal(module.nanoid(8).length, 8);
      assert.throws(() => Reflect.construct(String, [], module.nanoid), TypeError);
    },
  },
  {
    name: "mitt",
    input: join(root, "ports/mitt/index.lil"),
    functionSpelling: "function",
    properties: false,
    verify(module) {
      assert.equal(module.mitt.length, 1);
      assert.doesNotThrow(() => Reflect.construct(String, [], module.mitt));
      const emitter = module.mitt();
      let total = 0;
      emitter.on("value", (value) => {
        total += value;
      });
      emitter.emit("value", 3);
      assert.equal(total, 3);
      assert.deepEqual(Object.keys(emitter).sort(), ["all", "emit", "off", "on"]);
    },
  },
];

function config(library, variant) {
  return `[optimization]
preset="maximum"

[javascript]
priority="${variant.priority}"
optimization_level=${variant.level}
cost_model="brotli"
candidate_search="${variant.search}"
candidate_limit=${variant.limit}
candidate_beam_width=${variant.beam}
function_spelling="${library.functionSpelling}"

[mangle]
identifiers=true
properties=${library.properties}
pool_strings=true
`;
}

function metrics(code) {
  return {
    raw: code.length,
    gzip: gzipSync(code, { level: 9, mtime: 0 }).length,
    brotli: brotliCompressSync(code, {
      params: { [constants.BROTLI_PARAM_QUALITY]: 11 },
    }).length,
  };
}

const rows = [];
for (const library of libraries) {
  for (const variant of variants) {
    const base = `${library.name}-${variant.name}`;
    const configPath = join(outputRoot, `${base}.toml`);
    const outputPath = join(outputRoot, `${base}.mjs`);
    writeFileSync(configPath, config(library, variant));
    const started = performance.now();
    const result = spawnSync(
      compiler,
      [library.input, "--target", "js-module", "--config", configPath, "-o", outputPath],
      { cwd: repo, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 },
    );
    const milliseconds = performance.now() - started;
    if (result.status !== 0) throw new Error(result.stdout + result.stderr);
    const module = await import(`${pathToFileURL(outputPath).href}?audit=${Date.now()}`);
    library.verify(module);
    const code = readFileSync(outputPath);
    rows.push({
      library: library.name,
      variant: variant.name,
      priority: variant.priority,
      optimizationLevel: variant.level,
      candidateLimit: variant.limit,
      candidateBeamWidth: variant.beam,
      milliseconds,
      ...metrics(code),
    });
  }
}

const report = {
  node: process.version,
  note: "Wall time includes process startup, parsing, optimization, exact-codec search, and emission.",
  rows,
};
writeFileSync(join(root, "build/config-audit.json"), `${JSON.stringify(report, null, 2)}\n`);

const markdown = `# Exact-library configuration effort audit

Every artifact below passes its reusable-module API/behavior check. Times include
compiler process startup and are intended to show the tuning curve, not a stable
cross-machine compiler benchmark. Sizes are the emitted ESM module before Vite.

| Library | Profile | Priority / level | Candidate cap / beam | Compile ms | Raw | gzip-9 | Brotli-11 |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |
${rows
  .map(
    (row) =>
      `| ${row.library} | ${row.variant} | ${row.priority} / ${row.optimizationLevel} | ${row.candidateLimit} / ${row.candidateBeamWidth} | ${row.milliseconds.toFixed(1)} | ${row.raw} | ${row.gzip} | ${row.brotli} |`,
  )
  .join("\n")}
`;
writeFileSync(join(root, "CONFIG-AUDIT.md"), markdown);
console.log(markdown);
