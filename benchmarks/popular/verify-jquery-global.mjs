import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const labRoot = dirname(fileURLToPath(import.meta.url));
const globalPath = join(labRoot, "build/jquery-lilscript.global.js");
const { JSDOM } = await import("jsdom");

const code = readFileSync(globalPath, "utf8");
const dom = new JSDOM(
  `<!doctype html><html><body>
  <div id="app"><span class="x">hi</span></div>
  <script>${code.replace(/<\/script>/gi, "<\\/script>")}</script>
</body></html>`,
  { pretendToBeVisual: true, runScripts: "dangerously" },
);

const $ = dom.window.jQuery;
assert.equal(typeof $, "function", "window.jQuery must be a function");
assert.equal(dom.window.$, $, "window.$ alias");
assert.equal($("#app").length, 1);
assert.equal($(".x").text(), "hi");
assert.equal(typeof $.ajax, "function");
assert.equal(typeof $.fn.animate, "function");
assert.equal(typeof $.noConflict, "function");

const previous = $.noConflict(true);
assert.equal(previous, $);
assert.equal(dom.window.jQuery, undefined);
assert.equal(dom.window.$, undefined);

console.log("jquery-global:script-tag-api:ok");
