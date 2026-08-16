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
const outFile = join(buildRoot, "jquery-lilscript-effects.js");

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
const dom = new JSDOM("<!doctype html><html><body></body></html>", { url: "http://localhost/" });
globalThis.window = dom.window;
globalThis.document = dom.window.document;
const upstream = upstreamFactory(dom.window);
const lilModule = await import(outFile);
const lil = lilModule.jQuery;

function settle(ms = 30) {
  return new Promise((r) => setTimeout(r, ms));
}

function makeEl($, win, styleText = "") {
  const el = win.document.createElement("div");
  el.setAttribute("style", styleText);
  win.document.body.appendChild(el);
  return $(el);
}

async function runBoth(name, fn) {
  const outU = await fn(upstream, dom.window);
  const outL = await fn(lil, dom.window);
  assert.deepEqual(outL, outU, `${name}: lil !== upstream`);
  console.log(`jquery-effects:${name}:ok`);
}

await runBoth("show-hide-basic", async ($, win) => {
  const out = [];
  const el = makeEl($, win);
  out.push(["initial-display", el.css("display")]);
  el.hide();
  out.push(["after-hide", el.css("display"), el[0].style.display]);
  el.show();
  out.push(["after-show", el.css("display")]);
  el.remove();
  return out;
});

await runBoth("hide-inline-then-show-restores", async ($, win) => {
  const out = [];
  const el = makeEl($, win, "display: inline;");
  el.hide();
  out.push(["hidden", el[0].style.display]);
  el.show();
  out.push(["restored", el[0].style.display]);
  el.remove();
  return out;
});

await runBoth("toggle-noargs", async ($, win) => {
  const out = [];
  const el = makeEl($, win);
  el.toggle();
  out.push(["after-first-toggle", el.css("display")]);
  el.toggle();
  out.push(["after-second-toggle", el.css("display")]);
  el.remove();
  return out;
});

await runBoth("toggle-boolean-arg", async ($, win) => {
  const out = [];
  const el = makeEl($, win);
  el.toggle(false);
  out.push(["toggle-false", el.css("display")]);
  el.toggle(true);
  out.push(["toggle-true", el.css("display")]);
  el.remove();
  return out;
});

await runBoth("animate-single-prop-duration0", async ($, win) => {
  const out = [];
  const el = makeEl($, win, "opacity: 1;");
  const done = await new Promise((resolve) => {
    el.animate({ opacity: 0 }, 0, function () {
      resolve(this.style.opacity);
    });
  });
  out.push(["final-opacity", done]);
  out.push(["queue-length-after", el.queue("fx").length]);
  el.remove();
  return out;
});

await runBoth("animate-multi-prop-with-unit", async ($, win) => {
  const out = [];
  const el = makeEl($, win, "opacity: 1; left: 0px; position: absolute;");
  await new Promise((resolve) => {
    el.animate({ opacity: 0, left: 50 }, 0, resolve);
  });
  out.push(["opacity", el[0].style.opacity]);
  out.push(["left", el[0].style.left]);
  el.remove();
  return out;
});

await runBoth("fadeOut-then-fadeIn-duration0", async ($, win) => {
  const out = [];
  const el = makeEl($, win);
  await new Promise((resolve) => el.fadeOut(0, resolve));
  out.push(["after-fadeOut-display", el.css("display")]);
  out.push(["after-fadeOut-opacity", el[0].style.opacity]);
  await new Promise((resolve) => el.fadeIn(0, resolve));
  out.push(["after-fadeIn-display", el.css("display")]);
  out.push(["after-fadeIn-opacity", el[0].style.opacity]);
  el.remove();
  return out;
});

await runBoth("fadeTo-duration0", async ($, win) => {
  const out = [];
  const el = makeEl($, win);
  await new Promise((resolve) => el.fadeTo(0, 0.25, resolve));
  out.push(["opacity", el[0].style.opacity]);
  el.remove();
  return out;
});

await runBoth("fadeToggle-duration0-twice", async ($, win) => {
  const out = [];
  const el = makeEl($, win);
  await new Promise((resolve) => el.fadeToggle(0, resolve));
  out.push(["first-display", el.css("display")]);
  await new Promise((resolve) => el.fadeToggle(0, resolve));
  out.push(["second-display", el.css("display")]);
  el.remove();
  return out;
});

await runBoth("slideUp-slideDown-display", async ($, win) => {
  const out = [];
  const el = makeEl($, win);
  await new Promise((resolve) => el.slideUp(0, resolve));
  out.push(["after-slideUp-display", el.css("display")]);
  await new Promise((resolve) => el.slideDown(0, resolve));
  out.push(["after-slideDown-display", el.css("display")]);
  out.push(["overflow-restored", el[0].style.overflow]);
  el.remove();
  return out;
});

await runBoth("queue-false-restores-overflow-and-fires-complete", async ($, win) => {
  const out = [];
  const el = makeEl($, win, "height: 20px; overflow: scroll;");
  await new Promise((resolve) => {
    el.animate(
      { height: "hide" },
      {
        duration: 0,
        queue: false,
        complete() {
          out.push(["complete", el[0].style.overflow, el.css("display")]);
          resolve();
        },
      },
    );
  });
  out.push(["settled", el[0].style.overflow, el.queue("fx").length]);
  el.remove();
  return out;
});

await runBoth("animated-selector-mid-animation", async ($, win) => {
  const out = [];
  const el = makeEl($, win, "opacity: 1;");
  el.animate({ opacity: 0 }, 200);
  out.push(["animated-count-immediately", $(win.document).find("*").addBack().filter(":animated").length]);
  out.push(["is-animated-direct", $.contains ? el.is(":animated") : null]);
  el.stop(true);
  out.push(["animated-count-after-stop", el.is(":animated")]);
  el.remove();
  return out;
});

await runBoth("stop-no-gotoEnd-freezes-before-first-tick", async ($, win) => {
  const out = [];
  const el = makeEl($, win, "opacity: 1;");
  el.animate({ opacity: 0 }, 500);
  el.stop();
  out.push(["opacity-frozen", el[0].style.opacity]);
  out.push(["queue-length", el.queue("fx").length]);
  el.remove();
  return out;
});

await runBoth("stop-gotoEnd-jumps-to-final-value", async ($, win) => {
  const out = [];
  const el = makeEl($, win, "opacity: 1;");
  el.animate({ opacity: 0 }, 500);
  el.stop(true);
  out.push(["opacity-final", el[0].style.opacity]);
  el.remove();
  return out;
});

await runBoth("finish-clears-queue-and-jumps-to-last", async ($, win) => {
  const out = [];
  const el = makeEl($, win, "opacity: 1; left: 0px; position: absolute;");
  el.animate({ opacity: 0.5 }, 500);
  el.animate({ opacity: 0 }, 500);
  el.finish();
  out.push(["opacity-final", el[0].style.opacity]);
  out.push(["queue-length-after-finish", el.queue("fx").length]);
  el.remove();
  return out;
});

await runBoth("animate-queue-order-callbacks", async ($, win) => {
  const out = [];
  const el = makeEl($, win, "opacity: 1;");
  await new Promise((resolve) => {
    el.animate({ opacity: 0.5 }, 0, () => out.push("first"));
    el.animate({ opacity: 1 }, 0, () => {
      out.push("second");
      resolve();
    });
  });
  return out;
});

await runBoth("toggle-with-speed-uses-animate", async ($, win) => {
  const out = [];
  const el = makeEl($, win);
  out.push(["initial-display", el.css("display")]);
  await new Promise((resolve) => el.toggle(0, resolve));
  out.push(["after-toggle-hide-display", el.css("display")]);
  await new Promise((resolve) => el.toggle(0, resolve));
  out.push(["after-toggle-show-display", el.css("display")]);
  el.remove();
  return out;
});

await runBoth("stop-clearQueue-empties-fx-queue", async ($, win) => {
  const out = [];
  const el = makeEl($, win, "opacity: 1;");
  el.animate({ opacity: 0.6 }, 500);
  el.animate({ opacity: 0.2 }, 500);
  el.animate({ opacity: 0 }, 500);
  out.push(["queue-before-stop", el.queue("fx").length]);
  el.stop(true, true);
  out.push(["queue-after-clearQueue-stop", el.queue("fx").length]);
  el.remove();
  return out;
});

await runBoth("animate-relative-value-plus-equals", async ($, win) => {
  const out = [];
  const el = makeEl($, win, "left: 10px; position: absolute;");
  await new Promise((resolve) => {
    el.animate({ left: "+=15" }, 0, resolve);
  });
  out.push(["left-after-relative", el[0].style.left]);
  el.remove();
  return out;
});

await runBoth("fx-off-forces-instant-completion", async ($, win) => {
  const out = [];
  $.fx.off = true;
  const el = makeEl($, win, "opacity: 1;");
  await new Promise((resolve) => {
    el.animate({ opacity: 0 }, 500, resolve);
  });
  out.push(["opacity", el[0].style.opacity]);
  $.fx.off = false;
  el.remove();
  return out;
});

console.log("jquery-effects:all:ok");
