#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build as esbuild } from "esbuild";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const compiler = join(repoRoot, "target/release/lilscript");
const portRoot = join(labRoot, "ports/jquery");
const buildRoot = join(labRoot, "build");
mkdirSync(buildRoot, { recursive: true });

const mode = process.argv[2] || "all";

function run(program, args, cwd = labRoot) {
  const result = spawnSync(program, args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")}\n${result.stdout}${result.stderr}`);
  }
  return result.stdout.trim();
}

function compile(entryRel, configRel, rawOutRel) {
  run(compiler, [
    join(labRoot, entryRel),
    "--config",
    join(labRoot, configRel),
    "--target",
    "js-module",
    "-o",
    join(labRoot, rawOutRel),
  ]);
}

async function bundleEsm(rawRel, outRel) {
  await esbuild({
    absWorkingDir: portRoot,
    entryPoints: [join(labRoot, rawRel)],
    outfile: join(labRoot, outRel),
    bundle: true,
    format: "esm",
    platform: "neutral",
    write: true,
  });
}

async function bundleGlobal(rawRel, outRel) {
  const tmp = join(buildRoot, "_jquery-global-iife.js");
  await esbuild({
    absWorkingDir: portRoot,
    entryPoints: [join(labRoot, rawRel)],
    outfile: tmp,
    bundle: true,
    format: "iife",
    platform: "browser",
    write: true,
  });
  const body = readFileSync(tmp, "utf8");
  writeFileSync(
    join(labRoot, outRel),
    body +
      "\n;typeof window!==\"undefined\"&&window.jQuery&&(window.$=window.jQuery);\n",
  );
}

if (mode === "public" || mode === "all") {
  compile(
    "ports/jquery/entry.lil",
    "ports/jquery/lilscript.toml",
    "ports/jquery/jquery-lilscript.raw.js",
  );
  await bundleEsm("ports/jquery/jquery-lilscript.raw.js", "build/jquery-lilscript.mjs");
  compile(
    "ports/jquery/entry.lil",
    "ports/jquery/lilscript.toml",
    "ports/jquery/jquery-lilscript-global.raw.js",
  );
  await bundleGlobal(
    "ports/jquery/jquery-lilscript-global.raw.js",
    "build/jquery-lilscript.global.js",
  );
  console.log("public esm: build/jquery-lilscript.mjs");
  console.log("global script: build/jquery-lilscript.global.js (window.jQuery / window.$)");
}

if (mode === "app" || mode === "all") {
  compile(
    "ports/jquery/entry.lil",
    "ports/jquery/lilscript.app.toml",
    "ports/jquery/jquery-lilscript-app.raw.js",
  );
  await bundleEsm(
    "ports/jquery/jquery-lilscript-app.raw.js",
    "build/jquery-lilscript.app.mjs",
  );
  console.log("app-mangled: build/jquery-lilscript.app.mjs");
}
