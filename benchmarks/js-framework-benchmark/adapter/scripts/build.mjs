import {
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { spawnSync } from "node:child_process";

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

function moduleSpecifier(from, target) {
  const value = relative(from, target).replaceAll("\\", "/");
  return value.startsWith(".") ? value : `./${value}`;
}

function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    const detail = (result.stderr || result.stdout || "").trim();
    throw new Error(
      `${command} exited with status ${result.status}${detail ? `\n${detail}` : ""}`,
    );
  }
  return result.stdout;
}

const adapterRoot = resolve(import.meta.dirname, "..");
const sourceDirectory = resolve(adapterRoot, "src");
const repositoryRoot = findRepositoryRoot(adapterRoot);
const solidLab = resolve(repositoryRoot, "labs", "solid-client");
const directHost = resolve(
  solidLab,
  "packages",
  "solidlil",
  "internal",
  "direct-dom-host.js",
);
const { compileLilx } = await import(
  pathToFileURL(resolve(solidLab, "tooling", "lilx", "compile.mjs"))
);
const { createDirectDomWebSource } = await import(
  pathToFileURL(resolve(solidLab, "tooling", "lilx", "direct-dom.mjs"))
);
const { createEffectOnlyReactiveSource } = await import(
  pathToFileURL(resolve(solidLab, "tooling", "lilx", "direct-reactive.mjs"))
);

writeFileSync(
  resolve(sourceDirectory, "direct-dom-host.js"),
  `export * from ${JSON.stringify(moduleSpecifier(sourceDirectory, directHost))};\n`,
);

const reactiveModule = "./reactive-direct";
const applicationSource = readFileSync(resolve(sourceDirectory, "main.lilx"), "utf8");
const unsupportedEffectOnlyFeature = applicationSource.match(
  /\b(?:createMemo|createComputed|createResource|createSelector|createDeferred|createReaction|requestCallback)\b/,
);
if (unsupportedEffectOnlyFeature) {
  throw new Error(
    `The effect-only closed-world profile cannot compile ${unsupportedEffectOnlyFeature[0]}`,
  );
}
writeFileSync(
  resolve(sourceDirectory, "reactive-direct.lil"),
  createEffectOnlyReactiveSource(
    readFileSync(
      resolve(solidLab, "apps", "lilscript", "src", "reactive.lil"),
      "utf8",
    ),
  ),
);
const directWeb = createDirectDomWebSource(
  readFileSync(
    resolve(solidLab, "apps", "lilscript", "src", "web.lil"),
    "utf8",
  ),
  { errorBoundary: false, suspense: false },
).replace('from "./reactive"', `from ${JSON.stringify(reactiveModule)}`);
writeFileSync(resolve(sourceDirectory, "web-direct.lil"), directWeb);

const lilscript = compileLilx(
  applicationSource,
  {
    filename: resolve(sourceDirectory, "main.lilx"),
    reactiveImport: reactiveModule,
    domImport: "./web-direct",
    hostImport: "./direct-dom-host.js",
    directDom: true,
    persistentDelegation: true,
  },
);
const entry = resolve(sourceDirectory, "main.lil");
writeFileSync(entry, lilscript);

writeFileSync(
  resolve(adapterRoot, "index.html"),
  `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>SolidLil-keyed</title>
  <link href="/css/currentStyle.css" rel="stylesheet" />
</head>
<body>
  <div id="main"></div>
</body>
</html>
`,
);

const compiler = resolve(repositoryRoot, "target", "release", "lilscript");
if (!existsSync(compiler)) throw new Error(`Missing build tool ${compiler}`);

const javascript = run(
  compiler,
  [
    entry,
    "--target",
    "js-module",
    "--config",
    resolve(adapterRoot, "config", "closed-world.toml"),
  ],
  adapterRoot,
);
if (!javascript.trim()) throw new Error("Lilscript produced no JavaScript");
const outputDirectory = resolve(adapterRoot, "dist");
mkdirSync(outputDirectory, { recursive: true });
writeFileSync(resolve(outputDirectory, "main.js"), javascript);
const upstreamDist = resolve(
  repositoryRoot,
  "benchmarks",
  "js-framework-benchmark",
  "upstream",
  "frameworks",
  "keyed",
  "solidlil",
  "dist",
);
mkdirSync(upstreamDist, { recursive: true });
writeFileSync(resolve(upstreamDist, "main.js"), javascript);

writeFileSync(
  resolve(adapterRoot, "index.html"),
  `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>SolidLil-keyed</title>
  <link href="/css/currentStyle.css" rel="stylesheet" />
</head>
<body>
  <div id="main"></div>
  <script type="module" src="dist/main.js"></script>
</body>
</html>
`,
);
