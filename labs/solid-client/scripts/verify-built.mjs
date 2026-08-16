import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { build } from "vite";
import { chromium } from "../../../benchmarks/browser/playwright-runtime.mjs";
import { entryBundle, root } from "./project.mjs";

const markup =
  '<!doctype html><html><body><main id="app"></main></body></html>';

async function bundleCompilerHost() {
  const result = await build({
    configFile: false,
    logLevel: "silent",
    build: {
      write: false,
      target: "es2022",
      minify: false,
      lib: {
        entry: resolve(root, "apps/lilscript/src/lsx-host.js"),
        name: "SolidLilCompilerHost",
        formats: ["iife"],
      },
    },
  });
  const output = (Array.isArray(result) ? result[0] : result).output.find(
    ({ type }) => type === "chunk",
  );
  assert.ok(output, "compiler DOM host bundle");
  return output.code;
}

async function exerciseBundle(browser, label, path, prelude = "") {
  const context = await browser.newContext({ serviceWorkers: "block" });
  const page = await context.newPage();
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  try {
    await page.setContent(markup, { waitUntil: "load" });
    const source = `${prelude}\n${readFileSync(path, "utf8")}\n//# sourceURL=${label.replaceAll(" ", "-")}-playwright.js`;
    await page.evaluate((code) => (0, eval)(code), source);
    const result = await page.evaluate((label) => {
      const check = (condition, message) => {
        if (!condition) throw new Error(`${label}: ${message}`);
      };
      const node = (name) => document.querySelector(`[data-value="${name}"]`);
      const value = (name) => node(name)?.textContent;
      const click = (action) => {
        const button = document.querySelector(`[data-action="${action}"]`);
        check(button, `missing ${action} control`);
        button.click();
      };

      check(value("count") === "0", "initial count");
      check(value("doubled") === "0", "initial memo");
      click("increment");
      check(value("count") === "1", "increment");
      check(value("doubled") === "2", "memo update");
      check(value("parity") === "Odd", "derived parity");
      click("burst");
      check(value("count") === "101", "batched writes");
      check(value("doubled") === "202", "batched memo");
      click("reset");
      check(value("count") === "0", "reset");
      check(document.documentElement.dataset.count === "0", "effect");

      const staleButton = document.querySelector('[data-action="increment"]');
      check(
        typeof globalThis.__disposeSolidBenchmark === "function",
        "exposes root disposer",
      );
      globalThis.__disposeSolidBenchmark();
      globalThis.__disposeSolidBenchmark();
      check(document.querySelector("#app").childNodes.length === 0, "unmount");
      staleButton.click();
      check(
        document.documentElement.dataset.count === "0",
        "disposed handler updated state",
      );
      return {
        finalCount: document.documentElement.dataset.count,
        staleHandlersStopped: true,
        unmounted: true,
      };
    }, label);
    assert.deepEqual(pageErrors, [], `${label}: uncaught browser errors`);
    assert.deepEqual(result, {
      finalCount: "0",
      staleHandlersStopped: true,
      unmounted: true,
    });
    console.log(`${label} Playwright behavior passed.`);
  } finally {
    await context.close();
  }
}

const generated = resolve(root, "artifacts", "generated");
const sizeReportPath = resolve(root, "artifacts", "size-report.json");
const sizeReportBytes = readFileSync(sizeReportPath);
const sizeReport = JSON.parse(sizeReportBytes);
assert.equal(sizeReport.schemaVersion, 2, "current size report required");

const browser = await chromium.launch({ headless: true });
const browserVersion = browser.version();
try {
  await exerciseBundle(browser, "Solid Vite", entryBundle("solid"));
  await exerciseBundle(browser, "LilScript Vite", entryBundle("lilscript"));
  await exerciseBundle(
    browser,
    "Solid Closure ADVANCED",
    resolve(generated, "solid-closure-advanced.js"),
  );
  await exerciseBundle(
    browser,
    "LilScript Closure ADVANCED",
    resolve(generated, "lilscript-closure-advanced.js"),
  );
  await exerciseBundle(
    browser,
    "raw LilScript compiler output",
    resolve(generated, "lilscript-compiler.js"),
    await bundleCompilerHost(),
  );
} finally {
  await browser.close();
}

writeFileSync(
  resolve(root, "artifacts/app-behavior.json"),
  `${JSON.stringify(
    {
      schemaVersion: 1,
      generatedAt: new Date().toISOString(),
      environment: {
        automation: "Playwright 1.62.1",
        browser: "Chromium",
        browserVersion,
      },
      codecs: sizeReport.codecs,
      compiler: sizeReport.compiler,
      sizeEvidence: {
        path: "artifacts/size-report.json",
        sha256: createHash("sha256").update(sizeReportBytes).digest("hex"),
      },
      behaviorEquivalent: true,
      unmountVerified: true,
      staleHandlersStopped: true,
      artifacts: [
        "solid-vite",
        "lilscript-vite",
        "solid-closure-advanced",
        "lilscript-closure-advanced",
        "lilscript-compiler",
      ],
      artifactDigests: Object.fromEntries(
        [
          "solid-vite",
          "lilscript-vite",
          "solid-closure-advanced",
          "lilscript-closure-advanced",
          "lilscript-compiler",
        ].map((name) => [name, sizeReport.artifacts[name].sha256]),
      ),
    },
    null,
    2,
  )}\n`,
);
