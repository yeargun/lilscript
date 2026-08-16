import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";
import { build as esbuild } from "esbuild";
import { mkdirSync } from "node:fs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(labRoot, "../..");
const compiler = join(repoRoot, "target/release/lilscript");
const buildRoot = join(labRoot, "build");
const compiled = join(labRoot, "ports/jquery/jquery-lilscript.raw.js");
const outFile = join(buildRoot, "jquery-lilscript-selector-check.js");

mkdirSync(buildRoot, { recursive: true });

function run(program, args) {
  const result = spawnSync(program, args, {
    cwd: labRoot,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")}\n${result.stdout}${result.stderr}`);
  }
  return result.stdout.trim();
}

run(compiler, [
  join(labRoot, "ports/jquery/entry.lil"),
  "--mode",
  "development",
  "--target",
  "js-module",
  "-o",
  compiled,
]);

await esbuild({
  absWorkingDir: join(labRoot, "ports/jquery"),
  entryPoints: [compiled],
  outfile: outFile,
  bundle: true,
  format: "esm",
  platform: "neutral",
  write: true,
});

const require = createRequire(import.meta.url);
const upstreamFactory = require("jquery");
const { JSDOM } = await import("jsdom");
const dom = new JSDOM(`<!doctype html><html><body>
  <div id="app">
    <ul class="list">
      <li class="item first">One</li>
      <li class="item">Two</li>
      <li class="item">Three</li>
      <li class="item last">Four</li>
    </ul>
    <input type="text" id="txt" value="hi">
    <input type="checkbox" id="cb1" checked>
    <input type="checkbox" id="cb2">
    <input type="radio" name="r" id="r1">
    <div id="hidden-div" style="display:none">hidden content</div>
    <div id="visible-div">visible content</div>
    <p id="empty-p"></p>
    <p id="full-p">has text</p>
    <button id="btn1" disabled>Disabled</button>
    <button id="btn2">Enabled</button>
  </div>
</body></html>`, { pretendToBeVisual: true });
globalThis.window = dom.window;
globalThis.document = dom.window.document;
const upstream = upstreamFactory(dom.window);
const lil = await import(outFile);
const $u = upstream;
const $l = lil.jQuery;

function textsOf($col) {
  return $col.map(function () {
    return this.textContent;
  }).get();
}

function idsOf($col) {
  return $col.map(function () {
    return this.id;
  }).get();
}

const cases = [
  ["li.item", (col) => textsOf(col)],
  ["#app", (col) => idsOf(col)],
  [".list li:first", (col) => textsOf(col)],
  [".list li:last", (col) => textsOf(col)],
  [".list li:eq(1)", (col) => textsOf(col)],
  [".list li:even", (col) => textsOf(col)],
  [".list li:odd", (col) => textsOf(col)],
  [".list li:lt(2)", (col) => textsOf(col)],
  [".list li:gt(1)", (col) => textsOf(col)],
  ["li:not(.first)", (col) => textsOf(col)],
  ["#visible-div:visible", (col) => idsOf(col)],
  ["#hidden-div:hidden", (col) => idsOf(col)],
  ["#hidden-div:visible", (col) => idsOf(col)],
  [":checkbox", (col) => idsOf(col)],
  [":checked", (col) => idsOf(col)],
  [":radio", (col) => idsOf(col)],
  [":text", (col) => idsOf(col)],
  ["#empty-p:empty", (col) => idsOf(col)],
  ["#full-p:empty", (col) => idsOf(col)],
  ["p:parent", (col) => idsOf(col)],
  ["button:disabled", (col) => idsOf(col)],
  ["button:enabled", (col) => idsOf(col)],
  ["li:contains(Three)", (col) => textsOf(col)],
  ["#app :input", (col) => idsOf(col)],
  ["p:has(button)", (col) => idsOf(col)],
  ["ul.list li:eq(2) ~ li", (col) => textsOf(col)],
  ["div:has(#txt)", (col) => idsOf(col)],
  ["li:contains(One) + li", (col) => textsOf(col)],
  [":input:not(:checkbox):not(:radio)", (col) => idsOf(col)],
];

let failed = 0;
for (const [selector, extract] of cases) {
  const u = extract($u(selector));
  const l = extract($l(selector));
  try {
    assert.deepEqual(l, u, `selector mismatch for "${selector}"`);
    console.log(`OK   ${selector} -> ${JSON.stringify(l)}`);
  } catch (err) {
    failed += 1;
    console.log(`FAIL ${selector}\n  upstream: ${JSON.stringify(u)}\n  lil:      ${JSON.stringify(l)}`);
  }
}

// is()/filter()/not() through the traversing surface (may not be wired yet)
try {
  const isU = $u("#hidden-div").is(":hidden");
  const isL = $l.merge($l(), [dom.window.document.getElementById("hidden-div")]).is(":hidden");
  assert.equal(isL, isU);
  console.log(`OK   is(':hidden') -> ${isL}`);
} catch (err) {
  failed += 1;
  console.log(`FAIL is(':hidden'): ${err.message}`);
}

if (failed > 0) {
  console.log(`\n${failed} selector case(s) failed`);
  process.exit(1);
} else {
  console.log("\nall selector cases matched upstream");
}
