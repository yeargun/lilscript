import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { minify } from "terser";
import { canonicalCodecSizes } from "../../../codec-contract.mjs";

function findRepositoryRoot(start) {
  let current = resolve(start);
  while (true) {
    if (
      existsSync(resolve(current, "Cargo.toml")) &&
      existsSync(resolve(current, "tooling", "lilpack", "vite-runtime.mjs"))
    ) {
      return current;
    }
    const parent = dirname(current);
    if (parent === current) throw new Error("Unable to locate the LilScript repository root");
    current = parent;
  }
}

function javascriptFiles(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...javascriptFiles(path));
    else if (entry.isFile() && entry.name.endsWith(".js")) files.push(path);
  }
  return files;
}

function sizes(content) {
  const measured = canonicalCodecSizes(content, "framework compression variants");
  return {
    raw: measured.raw,
    gzip9: measured.gzip,
    brotli11: measured.brotli,
    sha256: createHash("sha256").update(content).digest("hex"),
  };
}

const adapterRoot = resolve(import.meta.dirname, "..");
const repositoryRoot = findRepositoryRoot(adapterRoot);
const entry = resolve(adapterRoot, "src", "main.lil");
const compiler = resolve(repositoryRoot, "target", "release", "lilscript");
const lilpack = resolve(repositoryRoot, "target", "release", "lilpack");
const vite = resolve(
  repositoryRoot,
  "tooling",
  "lilpack",
  "node_modules",
  "vite",
  "dist",
  "node",
  "index.js",
);
const outputRoot = resolve(
  adapterRoot,
  "..",
  "artifacts",
  "compression-variants",
);
const solid = readFileSync(
  resolve(adapterRoot, "..", "upstream", "frameworks", "keyed", "solid", "dist", "main.js"),
);
const variants = [
  { id: "size-first-no-terser", config: "closed-world.toml", terser: false },
  { id: "balanced", config: "closed-world-balanced.toml", terser: true },
  { id: "balanced-no-terser", config: "closed-world-balanced.toml", terser: false },
  { id: "realistic-performance-first", config: "closed-world-realistic-perf.toml", terser: true },
  { id: "performance-first", config: "closed-world-performance.toml", terser: true },
];

if (!existsSync(entry)) throw new Error("Compile the adapter once before measuring variants");
mkdirSync(outputRoot, { recursive: true });

const measuredSizeFirst = readFileSync(resolve(adapterRoot, "dist", "main.js"));
const results = [
  { id: "solid-js", ...sizes(solid) },
  { id: "size-first", terser: true, ...sizes(measuredSizeFirst) },
];
writeFileSync(resolve(outputRoot, "size-first.js"), measuredSizeFirst);
console.log(JSON.stringify(results[1]));

for (const variant of variants) {
  const intermediate = resolve(outputRoot, `.lilpack-${variant.id}`);
  rmSync(intermediate, { recursive: true, force: true });
  const result = spawnSync(
    lilpack,
    [
      "build",
      entry,
      "--root",
      adapterRoot,
      "--config",
      resolve(adapterRoot, "config", variant.config),
      "--base",
      "./",
      "--out-dir",
      intermediate,
      "--compiler",
      compiler,
      "--vite",
      vite,
    ],
    { cwd: adapterRoot, encoding: "utf8", stdio: "inherit" },
  );
  if (result.status !== 0) throw new Error(`lilpack failed for ${variant.id}`);
  const assets = javascriptFiles(intermediate);
  if (assets.length !== 1) throw new Error(`${variant.id}: expected one JS asset`);
  let code = readFileSync(assets[0], "utf8");
  if (variant.terser) {
    const minified = await minify(code, {
      module: true,
      compress: { passes: 3 },
      // This downstream comparison renames bindings, not object properties.
      mangle: { properties: false },
      format: { comments: false },
    });
    if (!minified.code) throw new Error(`Terser produced no JavaScript for ${variant.id}`);
    code = minified.code;
  }
  const buffer = Buffer.from(code);
  writeFileSync(resolve(outputRoot, `${variant.id}.js`), buffer);
  rmSync(intermediate, { recursive: true, force: true });
  const measured = { id: variant.id, terser: variant.terser, ...sizes(buffer) };
  results.push(measured);
  console.log(JSON.stringify(measured));
}

writeFileSync(
  resolve(outputRoot, "sizes.json"),
  `${JSON.stringify({ generatedAt: new Date().toISOString(), results }, null, 2)}\n`,
);
console.log(`wrote ${results.length} rows to ${outputRoot}`);
