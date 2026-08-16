import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { afterAll, describe, expect, test } from "vitest";
import { build } from "vite";
import { chromium } from "../../../benchmarks/browser/playwright-runtime.mjs";
import { compilerPath, projectRoot } from "../tooling/compiler-path.mjs";

let compiledPort;
let hostBundle;
let browser;

function compilePort() {
  if (compiledPort) return compiledPort;
  const output = resolve(tmpdir(), `lilscript-solid-test-${process.pid}.js`);
  const result = spawnSync(
    compilerPath(),
    [
      resolve(projectRoot, "apps/lilscript/src/main.lil"),
      "--target",
      "js",
      "-o",
      output,
    ],
    { encoding: "utf8", env: process.env },
  );
  if (result.status !== 0) throw new Error(result.stderr);
  compiledPort = readFileSync(output, "utf8");
  return compiledPort;
}

async function compileHostBundle() {
  if (hostBundle) return hostBundle;
  const result = await build({
    configFile: false,
    logLevel: "silent",
    build: {
      write: false,
      target: "es2022",
      minify: false,
      lib: {
        entry: resolve(projectRoot, "apps/lilscript/src/lsx-host.js"),
        name: "SolidLilReactiveHost",
        formats: ["iife"],
      },
    },
  });
  const output = (Array.isArray(result) ? result[0] : result).output.find(
    ({ type }) => type === "chunk",
  );
  hostBundle = output.code;
  return hostBundle;
}

async function browserInstance() {
  browser ??= await chromium.launch({ headless: true });
  return browser;
}

afterAll(async () => {
  await browser?.close();
});

describe("LilScript reactive port", () => {
  test("compiles modules into one executable without import wrappers", () => {
    const output = compilePort();
    expect(output).not.toContain("import ");
    expect(output).not.toContain("export ");
    expect(output).toContain("domQueryRoot");
    expect(output).toContain("hostSchedule");
  });

  test("matches the counter, memo, effect, and batch behavior in Chromium", async () => {
    const context = await (await browserInstance()).newContext();
    const page = await context.newPage();
    try {
      await page.setContent(
        '<!doctype html><html><body><main id="app"></main></body></html>',
        { waitUntil: "load" },
      );
      await page.evaluate(
        ({ host, source }) =>
          (0, eval)(
            `${host}\n${source}\n//# sourceURL=reactive-port-playwright.js`,
          ),
        { host: await compileHostBundle(), source: compilePort() },
      );
      const digest = await page.evaluate(() => {
        const node = (name) => document.querySelector(`[data-value="${name}"]`);
        const value = (name) => node(name).textContent;
        const click = (action) =>
          document.querySelector(`[data-action="${action}"]`).click();
        const initial = {
          count: value("count"),
          parity: value("parity"),
        };
        const countText = node("count").firstChild;
        click("increment");
        const incremented = {
          count: value("count"),
          doubled: value("doubled"),
          effect: document.documentElement.dataset.count,
          parity: value("parity"),
          textIdentityPreserved: node("count").firstChild === countText,
        };
        click("burst");
        const burst = {
          count: value("count"),
          doubled: value("doubled"),
        };
        click("reset");
        const reset = {
          count: value("count"),
          parity: value("parity"),
        };
        return { burst, incremented, initial, reset };
      });
      expect(digest).toEqual({
        initial: { count: "0", parity: "Even" },
        incremented: {
          count: "1",
          doubled: "2",
          effect: "1",
          parity: "Odd",
          textIdentityPreserved: true,
        },
        burst: { count: "101", doubled: "202" },
        reset: { count: "0", parity: "Even" },
      });
    } finally {
      await context.close();
    }
  });
});
