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
const outFile = join(buildRoot, "jquery-lilscript-traversing-check.js");

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

const html = `<!doctype html><html><body>
  <div id="app">
    <ul class="list">
      <li class="item first" data-x="1">One</li>
      <li class="item">Two</li>
      <li class="item">Three</li>
      <li class="item last">Four</li>
    </ul>
    <div id="a"><div id="b"><div id="c" class="target">deep</div></div></div>
    <span id="s1">s1</span><span id="s2">s2</span><span id="s3">s3</span>
  </div>
</body></html>`;

function freshDom() {
  const dom = new JSDOM(html, { pretendToBeVisual: true });
  return dom;
}

const domU = freshDom();
globalThis.window = domU.window;
globalThis.document = domU.window.document;
const $u = upstreamFactory(domU.window);

const domL = freshDom();
globalThis.window = domL.window;
globalThis.document = domL.window.document;
const lil = await import(outFile);
const $l = lil.jQuery;

function idsOf($col) {
  return $col.map(function () {
    return this.id;
  }).get();
}

function textsOf($col) {
  return $col.map(function () {
    return this.textContent;
  }).get();
}

const cases = [
  ["$(selector) direct construction", () => idsOf($l("li.item")), () => idsOf($u("li.item"))],
  ["$(selector) by id", () => idsOf($l("#app")), () => idsOf($u("#app"))],
  [".find('li')", () => idsOf($l("#app").find("li")), () => idsOf($u("#app").find("li"))],
  [".find(selector) with context", () => idsOf($l(".list").find("li.last")), () => idsOf($u(".list").find("li.last"))],
  [".filter('.item')", () => textsOf($l("li").filter(".item")), () => textsOf($u("li").filter(".item"))],
  [".not('.first')", () => textsOf($l("li").not(".first")), () => textsOf($u("li").not(".first"))],
  [".is(':first-child')", () => $l("li").eq(0).is(":first-child"), () => $u("li").eq(0).is(":first-child")],
  [".closest('#app')", () => idsOf($l("#c").closest("#app")), () => idsOf($u("#c").closest("#app"))],
  [".closest('.target')", () => idsOf($l("#b").closest(".target")), () => idsOf($u("#b").closest(".target"))],
  [".parent()", () => idsOf($l("#c").parent()), () => idsOf($u("#c").parent())],
  [".parents()", () => idsOf($l("#c").parents()), () => idsOf($u("#c").parents())],
  [".parentsUntil('#a')", () => idsOf($l("#c").parentsUntil("#a")), () => idsOf($u("#c").parentsUntil("#a"))],
  [".next()", () => idsOf($l("#s1").next()), () => idsOf($u("#s1").next())],
  [".prev()", () => idsOf($l("#s3").prev()), () => idsOf($u("#s3").prev())],
  [".nextAll()", () => idsOf($l("#s1").nextAll()), () => idsOf($u("#s1").nextAll())],
  [".prevAll()", () => idsOf($l("#s3").prevAll()), () => idsOf($u("#s3").prevAll())],
  [".nextUntil('#s3')", () => idsOf($l("#s1").nextUntil("#s3")), () => idsOf($u("#s1").nextUntil("#s3"))],
  [".siblings()", () => idsOf($l("#s2").siblings()), () => idsOf($u("#s2").siblings())],
  [".children()", () => idsOf($l("#app").children()), () => idsOf($u("#app").children())],
  [".has('.target')", () => idsOf($l("#a, #b, #app").has(".target")), () => idsOf($u("#a, #b, #app").has(".target"))],
  [".index() no arg", () => $l("#s2").index(), () => $u("#s2").index()],
  [".index(selector)", () => $l("#s2").index("span"), () => $u("#s2").index("span")],
  [".add(selector)", () => idsOf($l("#s1").add("#s3")), () => idsOf($u("#s1").add("#s3"))],
  [".addBack()", () => idsOf($l("#app").find("#s1").addBack()), () => idsOf($u("#app").find("#s1").addBack())],
  ["$(html string)", () => $l("<div class='fresh'>x</div>").hasClass("fresh"), () => $u("<div class='fresh'>x</div>").hasClass("fresh")],
  ["$(fn) ready shortcut", () => new Promise((resolveP) => $l(() => resolveP(true))), () => new Promise((resolveP) => $u(() => resolveP(true)))],
];

let failed = 0;
for (const [label, lilFn, upstreamFn] of cases) {
  try {
    const l = await lilFn();
    const u = await upstreamFn();
    assert.deepEqual(l, u, `mismatch for "${label}"`);
    console.log(`OK   ${label} -> ${JSON.stringify(l)}`);
  } catch (err) {
    failed += 1;
    console.log(`FAIL ${label}: ${err.message}`);
  }
}

if (failed > 0) {
  console.log(`\n${failed} traversing case(s) failed`);
  process.exit(1);
} else {
  console.log("\nall traversing cases matched upstream");
}
