import { spawnSync } from "node:child_process";
import { readFileSync, readdirSync, writeFileSync, mkdtempSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { build } from "vite";
import {
  canonicalCodecMeasurementsForFiles,
  canonicalCodecProvenance,
  canonicalCodecSizes,
} from "../../benchmarks/codec-contract.mjs";

const labRoot = resolve(fileURLToPath(new URL(".", import.meta.url)));
const lilastroRoot = resolve(labRoot, "..");
const repoRoot = resolve(lilastroRoot, "..");
const compiler = join(repoRoot, "target/release/lilscript");
const fixture = process.env.FIXTURE ?? "perf-stagger";
const entry = join(lilastroRoot, "browser", fixture, "lil", "main.lil");

const variants = [
  { name: "positional (current)", toml: null },
  {
    name: "named + prop mangling",
    toml: '[javascript]\naggregate_layout = "named"\n\n[mangle]\nproperties = true\n',
  },
  {
    name: "positional + prop mangling",
    toml: '[javascript]\naggregate_layout = "positional"\n\n[mangle]\nproperties = true\n',
  },
];

function sizes(code) {
  return canonicalCodecSizes(code, "Lilastro layout diagnostic");
}

function javascriptFiles(directory) {
  const paths = [];
  const visit = (current) => {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const path = join(current, entry.name);
      if (entry.isDirectory()) visit(path);
      else if (entry.isFile() && entry.name.endsWith(".js")) paths.push(path);
    }
  };
  visit(directory);
  return paths.sort();
}

function bundledSizes(directory) {
  const assets = javascriptFiles(directory);
  if (assets.length === 0) {
    throw new Error(
      `Lilastro layout diagnostic produced no JavaScript chunks in ${directory}`,
    );
  }
  const chunks = canonicalCodecMeasurementsForFiles(
    assets,
    "Lilastro layout diagnostic",
  );
  return {
    raw: chunks.reduce((sum, chunk) => sum + chunk.raw, 0),
    gzip: chunks.reduce((sum, chunk) => sum + chunk.gzip, 0),
    brotli: chunks.reduce((sum, chunk) => sum + chunk.brotli, 0),
    chunkCount: chunks.length,
  };
}

const dir = mkdtempSync(join(tmpdir(), "layout-"));
const results = [];

const lilDir = join(lilastroRoot, "browser", fixture, "lil");

for (const variant of variants) {
  const args = [entry, "--target", "js-module", "-o", join(lilDir, "main.js")];
  if (variant.toml) {
    const tomlPath = join(dir, "lilscript.toml");
    writeFileSync(tomlPath, variant.toml);
    args.push("--config", tomlPath);
  }
  const run = spawnSync(compiler, args, {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (run.status !== 0) {
    console.error(`FAILED ${variant.name}\n${run.stdout}\n${run.stderr}`);
    continue;
  }
  const source = readFileSync(join(lilDir, "main.js"), "utf8");
  const outDir = join(dir, `vite-${results.length}`);
  await build({
    root: lilDir,
    base: "./",
    logLevel: "error",
    build: {
      outDir,
      emptyOutDir: true,
      minify: true,
      rollupOptions: { input: join(lilDir, "index.html") },
    },
  });
  results.push({
    name: variant.name,
    compiler: sizes(source),
    minified: bundledSizes(outDir),
  });
}

const base = results[0];
console.log(`fixture: ${fixture}\n`);
console.log(
  `codec scorer: ${JSON.stringify(canonicalCodecProvenance("Lilastro layout diagnostic"))}\n`,
);
for (const result of results) {
  const ratio = (a, b) => `${(a / b).toFixed(4)}x`;
  console.log(result.name);
  console.log(
    `  compiler  raw=${result.compiler.raw} gzip=${result.compiler.gzip} brotli=${result.compiler.brotli}`,
  );
  console.log(
    `  minified  raw=${result.minified.raw} gzip=${result.minified.gzip} brotli=${result.minified.brotli} chunks=${result.minified.chunkCount}`,
  );
  if (result !== base) {
    console.log(
      `  vs base   raw=${ratio(result.minified.raw, base.minified.raw)} gzip=${ratio(result.minified.gzip, base.minified.gzip)} brotli=${ratio(result.minified.brotli, base.minified.brotli)}`,
    );
  }
  console.log("");
}
