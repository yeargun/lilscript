import { spawnSync } from "node:child_process";
import { mkdirSync, readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "vite";
import {
  canonicalCodecMeasurementsForFiles,
  requireCanonicalCodecRuntime,
} from "../codec-contract.mjs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const buildRoot = join(labRoot, "build");
const compiler = join(repoRoot, "target/release/lilscript");
mkdirSync(buildRoot, { recursive: true });
requireCanonicalCodecRuntime("Motion quick-size diagnostic");

function run(program, args, cwd = labRoot) {
  const result = spawnSync(program, args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(
      `${program} ${args.join(" ")}\n${result.stdout}${result.stderr}`,
    );
  }
  return result.stdout.trim();
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

async function viteSize(root, name) {
  const outDir = join(buildRoot, name);
  await build({
    root,
    logLevel: "error",
    build: {
      outDir,
      emptyOutDir: true,
      minify: true,
      write: true,
      rollupOptions: { input: join(root, "index.html") },
    },
  });
  const assets = javascriptFiles(outDir);
  if (assets.length === 0) {
    throw new Error(`Motion quick-size ${name} produced no JavaScript chunks`);
  }
  const chunks = canonicalCodecMeasurementsForFiles(
    assets,
    `Motion quick-size ${name}`,
  );
  return {
    raw: chunks.reduce((sum, chunk) => sum + chunk.raw, 0),
    gzip: chunks.reduce((sum, chunk) => sum + chunk.gzip, 0),
    brotli: chunks.reduce((sum, chunk) => sum + chunk.brotli, 0),
    chunkCount: chunks.length,
  };
}

run(compiler, [
  join(labRoot, "ports/motion/entry.lil"),
  "--target",
  "js-module",
  "-o",
  join(buildRoot, "motion-lilscript.js"),
]);
console.log(run(process.execPath, [join(labRoot, "verify-motion.mjs")]));

const npmVite = await viteSize(
  join(labRoot, "apps/motion/js"),
  "motion-vite-quick",
);
const lilVite = await viteSize(
  join(labRoot, "apps/motion/lil"),
  "motion-lilscript-vite-quick",
);
console.log("npm vite", npmVite);
console.log("lil vite", lilVite);
console.log(`brotli lil/npm ${lilVite.brotli}/${npmVite.brotli}`);
