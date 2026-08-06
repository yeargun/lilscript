import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { copyFile, mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { arch, homedir, platform, release } from "node:os";
import { dirname, extname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";
import { build as esbuild } from "esbuild";
import { build as viteBuild } from "vite";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const buildRoot = join(labRoot, "build");
const compiler = join(repoRoot, "target/release/lilscript");
const timingRunner = join(labRoot, "timing-runner.mjs");
const compatibilityPath = join(labRoot, "compatibility/libraries.json");
const webResults = join(repoRoot, "web/src/library-results.json");
const cargo = process.env.CARGO ?? join(homedir(), ".cargo/bin/cargo");
const closure = join(
  labRoot,
  "node_modules/.bin",
  platform() === "win32" ? "google-closure-compiler.cmd" : "google-closure-compiler",
);
const verifyOnly = process.argv.includes("--verify-only");
const warmups = verifyOnly ? 0 : Number(process.env.BENCH_WARMUPS ?? 5);
const samples = verifyOnly ? 1 : Number(process.env.BENCH_SAMPLES ?? 25);

const cases = [
  {
    id: "motion-easing",
    title: "Motion easing",
    scope: "Complete @motionone/easing root entrypoint",
    packages: ["@motionone/easing"],
    jsRoot: "apps/motion-easing/js",
    lilEntry: "apps/motion-easing/lil/main.lil",
    expected: "apps/motion-easing/expected.txt",
    portRoots: ["ports/motion-easing"],
  },
  {
    id: "micro-math",
    title: "Clamp and lerp",
    scope: "Complete clamp and lerp root entrypoints",
    packages: ["clamp", "lerp"],
    jsRoot: "apps/micro-math/js",
    lilEntry: "apps/micro-math/lil/main.lil",
    expected: "apps/micro-math/expected.txt",
    portRoots: ["ports/micro-math"],
  },
  {
    id: "string-hash",
    title: "String hash",
    scope: "Complete string-hash root entrypoint",
    packages: ["string-hash"],
    jsRoot: "apps/string-hash/js",
    lilEntry: "apps/string-hash/lil/main.lil",
    expected: "apps/string-hash/expected.txt",
    portRoots: ["ports/string-hash"],
  },
];

function command(program, args, options = {}) {
  const result = spawnSync(program, args, {
    cwd: options.cwd ?? repoRoot,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
    timeout: options.timeout ?? 240_000,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")} failed (${result.status})\n${result.stdout}${result.stderr}`);
  }
  return result.stdout.trim();
}

function normalize(value) {
  return value.replaceAll("\r\n", "\n").trimEnd();
}

function execute(path) {
  const result = spawnSync(process.execPath, [path], {
    cwd: labRoot,
    encoding: "utf8",
    maxBuffer: 4 * 1024 * 1024,
    timeout: 240_000,
  });
  if (result.error) throw result.error;
  if (result.status !== 0 || result.stderr) {
    throw new Error(`${relative(labRoot, path)} execution failed\n${result.stdout}${result.stderr}`);
  }
  return normalize(result.stdout);
}

function executeNative(path) {
  const result = spawnSync(path, [], {
    cwd: labRoot,
    encoding: "utf8",
    maxBuffer: 4 * 1024 * 1024,
    timeout: 240_000,
  });
  if (result.error) throw result.error;
  if (result.status !== 0 || result.stderr) {
    throw new Error(`${relative(labRoot, path)} native execution failed\n${result.stdout}${result.stderr}`);
  }
  return normalize(result.stdout);
}

async function filesUnder(directory) {
  const files = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await filesUnder(path)));
    else files.push(path);
  }
  return files.sort();
}

function metrics(contents) {
  const bytes = Buffer.isBuffer(contents) ? contents : Buffer.from(contents);
  return {
    raw: bytes.length,
    gzip: gzipSync(bytes, { level: 9, mtime: 0 }).length,
    brotli: brotliCompressSync(bytes, {
      params: { [constants.BROTLI_PARAM_QUALITY]: 11 },
    }).length,
  };
}

async function metricsForFiles(files) {
  const total = { raw: 0, gzip: 0, brotli: 0 };
  for (const file of files) {
    const measured = metrics(await readFile(file));
    total.raw += measured.raw;
    total.gzip += measured.gzip;
    total.brotli += measured.brotli;
  }
  return total;
}

async function sourceBytes(paths, extension) {
  let total = 0;
  for (const path of paths) {
    const files = (await filesUnder(path)).filter((file) => extname(file) === extension);
    for (const file of files) total += (await readFile(file)).length;
  }
  return total;
}

async function viteBundle(root, outDir) {
  await viteBuild({
    root,
    base: "./",
    configFile: false,
    logLevel: "silent",
    build: {
      outDir,
      emptyOutDir: true,
      manifest: true,
      minify: true,
      modulePreload: { polyfill: false },
      target: "baseline-widely-available",
    },
  });
  const manifest = JSON.parse(await readFile(join(outDir, ".vite/manifest.json"), "utf8"));
  const entry = Object.values(manifest).find((item) => item.isEntry);
  if (!entry) throw new Error(`Vite emitted no entry for ${root}`);
  const files = (await filesUnder(outDir)).filter((file) => !file.includes(`${join(".vite", "manifest.json")}`));
  const jsFiles = files.filter((file) => extname(file) === ".js");
  return {
    entry: join(outDir, entry.file),
    files: files.map((file) => relative(outDir, file)),
    js: await metricsForFiles(jsFiles),
    deploy: await metricsForFiles(files),
  };
}

async function referenceBundle(entry) {
  const result = await esbuild({
    absWorkingDir: labRoot,
    bundle: true,
    entryPoints: [entry],
    format: "iife",
    legalComments: "none",
    minify: false,
    platform: "browser",
    target: "es2021",
    treeShaking: true,
    write: false,
  });
  return result.outputFiles[0].text;
}

async function makeDeploy(directory, javascript) {
  await mkdir(directory, { recursive: true });
  const script = join(directory, "app.js");
  await copyFile(javascript, script);
  await writeFile(
    join(directory, "index.html"),
    '<!doctype html><html lang="en"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head><body><script src="./app.js"></script></body></html>\n',
  );
  const files = await filesUnder(directory);
  return {
    files: files.map((file) => relative(directory, file)),
    js: await metricsForFiles([script]),
    deploy: await metricsForFiles(files),
  };
}

function median(values) {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
}

function packageVersion(name) {
  const path = join(labRoot, "node_modules", ...name.split("/"), "package.json");
  return JSON.parse(readFileSync(path, "utf8")).version;
}

function percent(value, baseline) {
  const delta = (value / baseline - 1) * 100;
  return `${delta > 0 ? "+" : ""}${delta.toFixed(1)}%`;
}

function renderReport(report) {
  const lines = [
    "# Complete library compatibility results",
    "",
    `Generated ${report.metadata.generatedAt} from LilScript \`${report.metadata.compilerRevision}\` with Node \`${report.metadata.node}\`, Vite \`${report.metadata.vite}\`, esbuild \`${report.metadata.esbuild}\`, and Closure Compiler \`${report.metadata.closure}\`.`,
    "",
    "Each row executes the same checked app contract. The npm rows use the installed package, not a hand-specialized substitute. Closure receives an unminified esbuild bundle of that npm app because Closure does not install or resolve the package itself in this lab. LilScript also emits C and a native executable, and both must match before measurements are published.",
  ];
  for (const result of report.results) {
    const vite = result.artifacts.find((artifact) => artifact.id === "vite");
    lines.push(
      "",
      `## ${result.title}`,
      "",
      `Scope: **${result.scope}** using ${result.packages.map((item) => `\`${item.name}@${item.version}\``).join(" and ")}.`,
      "",
      `Contract: \`${result.expected}\``,
      "",
      `Translated upstream assertions: **${result.translatedAssertions}**. Monthly downloads at selection time: **${result.monthlyDownloads.toLocaleString("en-US")}**.`,
      "",
      "| Deployable JavaScript | Raw | Gzip-9 | Brotli-11 | vs npm/Vite Brotli | Median ms |",
      "| --- | ---: | ---: | ---: | ---: | ---: |",
    );
    for (const artifact of result.artifacts) {
      lines.push(`| ${artifact.label} | ${artifact.raw} | ${artifact.gzip} | ${artifact.brotli} | ${percent(artifact.brotli, vite.brotli)} | ${artifact.medianMs.toFixed(2)} |`);
    }
    lines.push(
      "",
      "| Full deploy (HTML + JS) | Raw | Gzip-9 | Brotli-11 |",
      "| --- | ---: | ---: | ---: |",
    );
    for (const artifact of result.artifacts) {
      lines.push(`| ${artifact.label} | ${artifact.deploy.raw} | ${artifact.deploy.gzip} | ${artifact.deploy.brotli} |`);
    }
  }
  lines.push(
    "",
    "## Limits",
    "",
    "- Complete means the documented callable root-entrypoint API for the statically typed input domain, not every accidental JavaScript coercion.",
    "- @motionone/easing is a complete published Motion ecosystem package; it is not motion@13 or its DOM engine.",
    "- Runtime measures cache-busted Node module parsing and deterministic app execution. It is not a browser rendering benchmark.",
    "- Transfer sizes sum independently compressed HTTP files. Source bytes are not used as shipping-size evidence.",
    "- A passing translated upstream suite and differential workload are strong regression evidence, not a mathematical proof over every input.",
    "",
  );
  return lines.join("\n");
}

await rm(buildRoot, { recursive: true, force: true });
await mkdir(buildRoot, { recursive: true });
if (existsSync(cargo)) command(cargo, ["build", "--release", "--bin", "lilscript"]);
else if (!existsSync(compiler)) throw new Error("Cargo and target/release/lilscript are unavailable");

const portModuleRoot = join(buildRoot, "ports");
await mkdir(portModuleRoot, { recursive: true });
for (const [source, output] of [
  ["ports/motion-easing/index.lil", "motion-easing.mjs"],
  ["ports/micro-math/clamp.lil", "clamp.mjs"],
  ["ports/micro-math/lerp.lil", "lerp.mjs"],
  ["ports/string-hash/index.lil", "string-hash.mjs"],
]) {
  command(compiler, [join(labRoot, source), "--target", "js-module", "-o", join(portModuleRoot, output)]);
}

const compatibility = JSON.parse(await readFile(compatibilityPath, "utf8"));
const results = [];
for (const benchmark of cases) {
  const directory = join(buildRoot, benchmark.id);
  await mkdir(directory, { recursive: true });
  const expected = normalize(await readFile(join(labRoot, benchmark.expected), "utf8"));
  const jsRoot = join(labRoot, benchmark.jsRoot);
  const vite = await viteBundle(jsRoot, join(directory, "vite"));

  const readableBundle = await referenceBundle(join(benchmark.jsRoot, "main.js"));
  const closureInput = join(directory, "closure-input.js");
  const closureOutput = join(directory, "closure.js");
  await writeFile(closureInput, readableBundle);
  command(closure, [
    "--js", closureInput,
    "--js_output_file", closureOutput,
    "--compilation_level", "ADVANCED",
    "--language_in", "ECMASCRIPT_2021",
    "--language_out", "ECMASCRIPT_2021",
    "--warning_level", "QUIET",
    "--emit_use_strict=false",
    "--rewrite_polyfills=false",
  ]);
  const closureDeploy = await makeDeploy(join(directory, "closure-deploy"), closureOutput);

  const lilBase = join(directory, "lilscript");
  command(compiler, [join(labRoot, benchmark.lilEntry), "--target", "all", "-o", lilBase]);
  const lilJs = `${lilBase}.js`;
  const lilDeploy = await makeDeploy(join(directory, "lilscript-deploy"), lilJs);

  const observed = [
    ["npm-vite", execute(vite.entry)],
    ["npm-closure", execute(closureOutput)],
    ["lilscript-js", execute(lilJs)],
    ["lilscript-native", executeNative(lilBase)],
  ];
  for (const [label, actual] of observed) {
    if (actual !== expected) {
      throw new Error(`${benchmark.id}/${label} mismatch\nexpected ${JSON.stringify(expected)}\nactual   ${JSON.stringify(actual)}`);
    }
  }

  const timing = JSON.parse(command(process.execPath, [
    timingRunner,
    String(warmups),
    String(samples),
    vite.entry,
    closureOutput,
    lilJs,
  ], { cwd: labRoot }));
  const port = compatibility.ports.find((item) => item.id === benchmark.id);
  const artifacts = [
    {
      id: "vite",
      label: "Installed npm package + Vite",
      ...vite.js,
      deploy: vite.deploy,
      files: vite.files,
      medianMs: median(timing[0]),
    },
    {
      id: "closure",
      label: "Installed npm package + Closure ADVANCED",
      ...closureDeploy.js,
      deploy: closureDeploy.deploy,
      files: closureDeploy.files,
      medianMs: median(timing[1]),
    },
    {
      id: "lilscript",
      label: "LilScript port",
      ...lilDeploy.js,
      deploy: lilDeploy.deploy,
      files: lilDeploy.files,
      medianMs: median(timing[2]),
      nativeVerified: true,
      cEmitted: existsSync(`${lilBase}.c`),
    },
  ];
  results.push({
    id: benchmark.id,
    title: benchmark.title,
    scope: benchmark.scope,
    packages: benchmark.packages.map((name) => ({ name, version: packageVersion(name) })),
    monthlyDownloads: port.monthlyDownloads,
    translatedAssertions: port.translatedAssertions,
    expected,
    source: {
      npmApp: await sourceBytes([jsRoot], ".js"),
      lilscriptAppAndPort: await sourceBytes([
        dirname(join(labRoot, benchmark.lilEntry)),
        ...benchmark.portRoots.map((root) => join(labRoot, root)),
      ], ".lil"),
    },
    artifacts,
  });
}

const packageJson = JSON.parse(await readFile(join(labRoot, "package.json"), "utf8"));
const report = {
  metadata: {
    generatedAt: new Date().toISOString(),
    compilerRevision: command("git", ["rev-parse", "--short", "HEAD"]),
    node: process.version,
    vite: packageJson.devDependencies.vite,
    esbuild: packageJson.devDependencies.esbuild,
    closure: packageJson.devDependencies["google-closure-compiler"],
    system: `${platform()} ${release()} ${arch()}`,
    warmups,
    samples,
    downloadWindow: compatibility.downloadWindow,
  },
  eligibilityRule: compatibility.eligibilityRule,
  results,
  auditedButIneligible: compatibility.auditedButIneligible,
};

await writeFile(join(buildRoot, "results.json"), `${JSON.stringify(report, null, 2)}\n`);
if (!verifyOnly) {
  await writeFile(join(labRoot, "RESULTS.md"), `${renderReport(report)}\n`);
  await writeFile(webResults, `${JSON.stringify(report, null, 2)}\n`);
}
console.log(`Verified ${results.length} complete library apps across npm/Vite, npm/Closure, LilScript JS, C, and native.`);
