import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const webRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(webRoot, "..");
const publicDemos = join(webRoot, "public/demos");
const mapPath = join(webRoot, "src/demo-preview-map.json");
const captureTag =
  '<script src="/demos/console-frame.js"></script>\n';

const popularIds = [
  "nanoid",
  "mitt",
  "clsx",
  "immer",
  "redux-toolkit",
  "zod",
  "acorn",
  "preact",
  "gl-matrix",
  "motion",
  "jquery",
];
const portIds = [
  "micro-math",
  "string-hash",
  "js-levenshtein",
  "emotion-hash",
  "murmurhash-js",
  "robust-predicates",
  "motion-easing",
];

function injectCapture(html) {
  if (html.includes("/demos/console-frame.js")) return html;
  if (/<head[^>]*>/i.test(html)) {
    return html.replace(/<head[^>]*>/i, (tag) => `${tag}\n${captureTag}`);
  }
  return `${captureTag}${html}`;
}

function isConsoleOnly(html) {
  const body = html.match(/<body[^>]*>([\s\S]*)<\/body>/i)?.[1] ?? "";
  const visible = body
    .replace(/<script\b[\s\S]*?<\/script>/gi, "")
    .replace(/<script\b[^>]*>/gi, "")
    .replace(/\s+/g, "");
  return visible.length === 0;
}

function copyWrapped(from, to) {
  if (!existsSync(join(from, "index.html"))) return false;
  rmSync(to, { recursive: true, force: true });
  mkdirSync(to, { recursive: true });
  cpSync(from, to, { recursive: true });
  const indexPath = join(to, "index.html");
  const html = readFileSync(indexPath, "utf8");
  if (isConsoleOnly(html)) writeFileSync(indexPath, injectCapture(html));
  return true;
}

function writeScriptPage(to, title, script, prelude = "") {
  rmSync(to, { recursive: true, force: true });
  mkdirSync(to, { recursive: true });
  writeFileSync(join(to, "main.js"), script);
  writeFileSync(
    join(to, "index.html"),
    `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>${title}</title>
    ${captureTag}
    ${prelude}
  </head>
  <body>
    <script src="./main.js"></script>
  </body>
</html>
`,
  );
}

const map = {};

for (const id of popularIds) {
  const js = join(repoRoot, "benchmarks/popular/build", `${id}-vite`);
  const lil = join(repoRoot, "benchmarks/popular/build", `${id}-lilscript-vite`);
  const jsOk = copyWrapped(js, join(publicDemos, "libs", `${id}-js`));
  const lilOk = copyWrapped(lil, join(publicDemos, "libs", `${id}-lil`));
  if (jsOk && lilOk) {
    map[`lib-${id}`] = {
      baseline: `/demos/libs/${id}-js/index.html`,
      candidate: `/demos/libs/${id}-lil/index.html`,
    };
  }
}

for (const id of portIds) {
  const js = join(repoRoot, "benchmarks/libraries/build", id, "vite");
  const lil = join(repoRoot, "benchmarks/libraries/build", id, "lilscript-deploy");
  const jsOk = copyWrapped(js, join(publicDemos, "ports", `${id}-js`));
  const lilOk = copyWrapped(lil, join(publicDemos, "ports", `${id}-lil`));
  if (jsOk && lilOk) {
    map[`port-${id}`] = {
      baseline: `/demos/ports/${id}-js/index.html`,
      candidate: `/demos/ports/${id}-lil/index.html`,
    };
  }
}

const algorithmResults = JSON.parse(
  readFileSync(join(webRoot, "src/algorithm-demo-results.json"), "utf8"),
);
for (const result of algorithmResults.cases) {
  const caseJson = JSON.parse(
    readFileSync(
      join(repoRoot, "comparison/algorithms/cases", result.id, "case.json"),
      "utf8",
    ),
  );
  const vector = caseJson.vectors[0];
  const prelude = `<script>window.algorithmInt=function(i){const v=${JSON.stringify(vector.ints ?? [])};if(i<0||i>=v.length)throw new RangeError(i);return v[i]};window.algorithmString=function(i){const v=${JSON.stringify(vector.strings ?? [])};if(i<0||i>=v.length)throw new RangeError(i);return v[i]};window.algorithmCount=function(){return Math.max(${(vector.ints ?? []).length},${(vector.strings ?? []).length})};</script>`;
  const baselineJs = join(
    repoRoot,
    "comparison/algorithms/build",
    result.id,
    `${result.baseline.id}.js`,
  );
  const lilJs = join(
    repoRoot,
    "comparison/algorithms/build",
    result.id,
    "lilscript-brotli.js",
  );
  if (!existsSync(baselineJs) || !existsSync(lilJs)) continue;
  writeScriptPage(
    join(publicDemos, "algo", `${result.id}-js`),
    result.baseline.id,
    readFileSync(baselineJs),
    prelude,
  );
  writeScriptPage(
    join(publicDemos, "algo", `${result.id}-lil`),
    "LilScript",
    readFileSync(lilJs),
    prelude,
  );
  map[`algo-${result.id}`] = {
    baseline: `/demos/algo/${result.id}-js/index.html`,
    candidate: `/demos/algo/${result.id}-lil/index.html`,
  };
}

const paired = JSON.parse(
  readFileSync(join(webRoot, "src/paired-results.json"), "utf8"),
);
for (const result of paired.results) {
  const baselineJs = join(repoRoot, "benchmarks/paired/build", result.id, "closure.js");
  const lilJs = join(repoRoot, "benchmarks/paired/build", result.id, "lilscript.js");
  if (!existsSync(baselineJs) || !existsSync(lilJs)) continue;
  writeScriptPage(
    join(publicDemos, "paired", `${result.id}-js`),
    "Closure ADVANCED",
    readFileSync(baselineJs),
  );
  writeScriptPage(
    join(publicDemos, "paired", `${result.id}-lil`),
    "LilScript",
    readFileSync(lilJs),
  );
  map[`paired-${result.id}`] = {
    baseline: `/demos/paired/${result.id}-js/index.html`,
    candidate: `/demos/paired/${result.id}-lil/index.html`,
  };
}

writeFileSync(mapPath, `${JSON.stringify(map, null, 2)}\n`);
console.log(`published ${Object.keys(map).length} demo preview pairs`);
