import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build as esbuild, transform as esbuildTransform } from "esbuild";
import { minify as terserMinify } from "terser";
import { canonicalCodecSizesForFile } from "../../codec-contract.mjs";
import { coreEsm, labRoot, lilPath, monacoPath } from "./file-map.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const compiler = process.env.LILSCRIPT
  ? resolve(process.cwd(), process.env.LILSCRIPT)
  : join(repoRoot, "target/release/lilscript");
const configPath = join(here, "lilscript.toml");

export const jsHostPlugin = {
  name: "monaco-js-host",
  setup(build) {
    build.onResolve({ filter: /(^|\/)js-host(\.ts)?$/ }, () => ({
      path: join(labRoot, "ports/monaco/js-host.ts"),
    }));
  },
};

export function runCompiler(entry, outPath) {
  const result = spawnSync(
    compiler,
    [entry, "--config", configPath, "--target", "js-module", "-o", outPath],
    { cwd: labRoot, encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
  );
  if (result.status !== 0) {
    throw new Error(`${compiler} ${entry}\n${result.stdout}${result.stderr}`);
  }
}

export function externalPlugin(filters) {
  return {
    name: "file-pair-externals",
    setup(build) {
      for (const filter of filters) {
        build.onResolve({ filter }, (args) => ({ path: args.path, external: true }));
      }
    },
  };
}

async function bundleToFile(options) {
  const result = await esbuild({
    bundle: true,
    format: "esm",
    platform: "neutral",
    minify: false,
    write: true,
    logOverride: { "import-is-undefined": "silent" },
    ...options,
  });
  return result;
}

async function minifyJsLanes(source, filename) {
  if (typeof source !== "string" || source.length === 0) {
    throw new Error(`${filename} is empty`);
  }
  const [esbuildResult, terserResult] = await Promise.all([
    esbuildTransform(source, {
      sourcefile: filename,
      loader: "js",
      format: "esm",
      target: "esnext",
      minify: true,
      legalComments: "none",
    }),
    terserMinify(source, {
      module: true,
      compress: { passes: 3 },
      mangle: true,
      format: { comments: false },
    }),
  ]);
  const esbuildCode = esbuildResult.code;
  const terserCode = terserResult.code;
  if (!esbuildCode) throw new Error(`esbuild produced no code for ${filename}`);
  if (!terserCode) throw new Error(`terser produced no code for ${filename}`);
  return { esbuild: esbuildCode, terser: terserCode };
}

export async function bestMinifiedSizes(source, filename, outDir) {
  mkdirSync(outDir, { recursive: true });
  const minified = await minifyJsLanes(source, filename);
  let best = null;
  const lanes = {};
  for (const [lane, code] of Object.entries(minified)) {
    const path = join(outDir, `${filename}.${lane}.js`);
    writeFileSync(path, code);
    const sizes = canonicalCodecSizesForFile(path, `${filename} ${lane}`);
    lanes[lane] = sizes;
    if (!best || sizes.brotli < best.sizes.brotli) {
      best = { lane, path, sizes };
    }
  }
  return { lanes, best };
}

export async function compileLilPair(pair, outDir) {
  mkdirSync(outDir, { recursive: true });
  const compiledPath = join(outDir, "lilscript.raw.js");
  runCompiler(lilPath(pair.lilEntry), compiledPath);
  const compiledSource = readFileSync(compiledPath, "utf8");
  const needsHost = /from\s*["'][^"']*js-host/u.test(compiledSource);
  let artifact = compiledPath;
  if (needsHost) {
    artifact = join(outDir, "lilscript.bundle.js");
    await bundleToFile({
      absWorkingDir: join(labRoot, "ports/monaco"),
      entryPoints: [compiledPath],
      outfile: artifact,
      plugins: [jsHostPlugin],
    });
  }
  return {
    path: artifact,
    bundledHost: needsHost,
    sizes: canonicalCodecSizesForFile(artifact, `pair ${pair.id} lilscript`),
  };
}

export async function compileJsPair(pair, outDir) {
  mkdirSync(outDir, { recursive: true });
  const bundledPath = join(outDir, "javascript.bundle.js");
  const plugins = [];
  if (pair.jsExternal?.length) {
    plugins.push(externalPlugin(pair.jsExternal));
  }
  if (pair.jsWrapper) {
    await bundleToFile({
      stdin: {
        contents: pair.jsWrapper,
        resolveDir: labRoot,
        sourcefile: `${pair.id}.js-pair.js`,
        loader: "js",
      },
      outfile: bundledPath,
      plugins,
    });
  } else {
    await bundleToFile({
      absWorkingDir: coreEsm,
      entryPoints: [monacoPath(pair.jsEntry)],
      outfile: bundledPath,
      plugins,
    });
  }
  const source = readFileSync(bundledPath, "utf8");
  const minified = await bestMinifiedSizes(source, pair.id, join(outDir, "minify"));
  return {
    bundledPath,
    bundledRaw: source.length,
    minified,
  };
}

export async function measurePair(pair, outDir) {
  const lil = await compileLilPair(pair, join(outDir, "lil"));
  const js = await compileJsPair(pair, join(outDir, "js"));
  return {
    id: pair.id,
    title: pair.title,
    plugged: pair.plugged,
    monacoFiles: pair.monacoFiles,
    lilFiles: pair.lilFiles,
    note: pair.note,
    lil: {
      path: lil.path.slice(repoRoot.length + 1),
      bundledHost: lil.bundledHost,
      sizes: lil.sizes,
    },
    js: {
      lane: js.minified.best.lane,
      bundledRaw: js.bundledRaw,
      sizes: js.minified.best.sizes,
      lanes: js.minified.lanes,
    },
    delta: {
      brotli: lil.sizes.brotli - js.minified.best.sizes.brotli,
    },
  };
}

export function scoreProductionFile(path, context) {
  return canonicalCodecSizesForFile(path, context);
}
