import { spawnSync } from "node:child_process";
import { readFileSync, rmSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterAll, describe, expect as vitestExpect, test } from "vitest";
import { build } from "vite";
import solid from "vite-plugin-solid";
import { chromium } from "../../../benchmarks/browser/playwright-runtime.mjs";
import { compileLilx } from "../tooling/lilx/compile.mjs";
import { compilerPath, projectRoot } from "../tooling/compiler-path.mjs";
import { entryBundle } from "../scripts/project.mjs";

let compiled;
let solidCompiled;
let solidlilHostCompiled;
let browser;

// Keep the browser callback source independent from Vitest's SSR import
// rewriting. In Node this forwards to Vitest; in Chromium it resolves to the
// matcher installed by mountSource.
const expect = (...arguments_) => vitestExpect(...arguments_);

const fixtureMarkup =
  '<!doctype html><html><head></head><body><main id="app"></main><aside id="portal-target"></aside><svg id="svg-portal-target"></svg><aside id="shadow-portal-target"></aside></body></html>';

async function browserInstance() {
  browser ??= await chromium.launch({ headless: true });
  return browser;
}

afterAll(async () => {
  await browser?.close();
});

function compileFixture() {
  if (compiled) return compiled;
  const input = resolve(projectRoot, "tests/lil/lsx-runtime.lilx");
  const generated = resolve(
    projectRoot,
    "tests/lil",
    `.solidlil-lsx-${process.pid}.lil`,
  );
  const output = resolve(
    projectRoot,
    "tests/lil",
    `.solidlil-lsx-${process.pid}.js`,
  );
  writeFileSync(
    generated,
    compileLilx(readFileSync(input, "utf8"), {
      filename: input,
      reactiveImport: "../../apps/lilscript/src/reactive",
      domImport: "../../apps/lilscript/src/web",
    }),
  );
  try {
    const result = spawnSync(
      compilerPath(),
      [generated, "--target", "js", "-o", output],
      { cwd: projectRoot, encoding: "utf8", env: process.env },
    );
    if (result.status !== 0) throw new Error(result.stderr || result.stdout);
    compiled = readFileSync(output, "utf8");
  } finally {
    rmSync(generated, { force: true });
    rmSync(output, { force: true });
  }
  return compiled;
}

async function compileSolidFixture() {
  if (solidCompiled) return solidCompiled;
  const result = await build({
    configFile: false,
    logLevel: "silent",
    plugins: [solid()],
    build: {
      write: false,
      target: "es2022",
      minify: false,
      lib: {
        entry: resolve(projectRoot, "tests/solid/lsx-runtime.jsx"),
        name: "SolidLsxFixture",
        formats: ["iife"],
      },
    },
  });
  const output = (Array.isArray(result) ? result[0] : result).output.find(
    ({ type }) => type === "chunk",
  );
  solidCompiled = output.code;
  return solidCompiled;
}

async function compileSolidLilHost() {
  if (solidlilHostCompiled) return solidlilHostCompiled;
  const result = await build({
    configFile: false,
    logLevel: "silent",
    build: {
      write: false,
      target: "es2022",
      minify: false,
      lib: {
        entry: resolve(projectRoot, "apps/lilscript/src/lsx-host.js"),
        name: "SolidLilLsxHost",
        formats: ["iife"],
      },
    },
  });
  const output = (Array.isArray(result) ? result[0] : result).output.find(
    ({ type }) => type === "chunk",
  );
  solidlilHostCompiled = output.code;
  return solidlilHostCompiled;
}

async function mountSource(kind, source) {
  const context = await (await browserInstance()).newContext();
  const page = await context.newPage();
  await page.setContent(fixtureMarkup, { waitUntil: "load" });
  await page.evaluate((runtimeKind) => {
    globalThis.__lsxRuntimeKind = runtimeKind;
    globalThis.close = () => {};
    globalThis.registerLsxDispose = (dispose) => {
      globalThis.__disposeLsx = dispose;
    };
    globalThis.registerLsxDiagnostics = (
      ownerSlots,
      effectSlots,
      freeOwnerSlots,
      freeEffectSlots,
      pendingEffects,
    ) => {
      globalThis.__lsxDiagnostics = () => ({
        owners: ownerSlots(),
        effects: effectSlots(),
        freeOwners: freeOwnerSlots(),
        freeEffects: freeEffectSlots(),
        pendingEffects: pendingEffects(),
      });
    };
    globalThis.registerLsxBoundaryDiagnostics = (
      boundaryCleanups,
      initialBoundaryCleanups,
      suspenseContentCleanups,
      suspenseFallbackCleanups,
    ) => {
      globalThis.__lsxBoundaryDiagnostics = () => ({
        boundaryCleanups: boundaryCleanups(),
        initialBoundaryCleanups: initialBoundaryCleanups(),
        suspenseContentCleanups: suspenseContentCleanups(),
        suspenseFallbackCleanups: suspenseFallbackCleanups(),
      });
    };

    const format = (value) => {
      if (value instanceof globalThis.Node)
        return value.outerHTML ?? value.nodeName;
      try {
        return JSON.stringify(value);
      } catch {
        return String(value);
      }
    };
    const equal = (left, right) =>
      JSON.stringify(left) === JSON.stringify(right);
    globalThis.expect = (actual, message = "browser expectation") => {
      const matchers = (inverted) => {
        const verify = (matches, expected, matcher) => {
          const passed = inverted ? !matches : matches;
          if (!passed) {
            throw new Error(
              `${message}: expected ${format(actual)} ${inverted ? "not " : ""}${matcher} ${format(expected)}`,
            );
          }
        };
        const api = {
          toBe: (expected) => verify(actual === expected, expected, "to be"),
          toBeNull: () => verify(actual === null, null, "to be null"),
          toEqual: (expected) =>
            verify(equal(actual, expected), expected, "to equal"),
          toHaveLength: (expected) =>
            verify(actual?.length === expected, expected, "to have length"),
        };
        Object.defineProperty(api, "not", {
          get: () => matchers(!inverted),
        });
        return api;
      };
      return matchers(false);
    };
  }, kind);
  await page.evaluate(
    ({ kind, source }) =>
      (0, eval)(`${source}\n//# sourceURL=lsx-${kind}-playwright.js`),
    { kind, source },
  );
  return { context, page };
}

async function mount(kind) {
  const source =
    kind === "solidlil"
      ? `${await compileSolidLilHost()}\n${compileFixture()}`
      : await compileSolidFixture();
  return mountSource(kind, source);
}

async function mountBuilt(kind) {
  return mountSource(
    `${kind}-built`,
    readFileSync(
      entryBundle(kind === "solidlil" ? "lsx-lilscript" : "lsx-solid"),
      "utf8",
    ),
  );
}

async function exerciseInBrowser(mounted, callback) {
  try {
    return await mounted.page.evaluate(
      async (source) => (0, eval)(`(${source})`)({ window: globalThis }),
      callback.toString(),
    );
  } finally {
    await mounted.context.close();
  }
}

async function exercise(dom) {
  const { document, Event, MouseEvent } = dom.window;
  const action = (name) => document.querySelector(`[data-action="${name}"]`);
  const click = (name) =>
    action(name).dispatchEvent(new MouseEvent("click", { bubbles: true }));
  try {
    const root = document.querySelector("#lsx-root");
    const input = document.querySelector("input");
    expect(root.dataset.count).toBe("0");
    expect(root.classList.contains("active")).toBe(true);
    expect(input.required).toBe(true);
    expect(input.value).toBe("0");
    expect(input.checked).toBe(true);
    expect(document.querySelector('[data-ref="captured"]')).not.toBeNull();
    expect(document.querySelector('[data-directive="ready"]')).not.toBeNull();
    expect(document.querySelector('[data-state="shown"]')).not.toBeNull();
    expect(document.querySelector('[data-nested="prefix"]')).not.toBeNull();
    expect(document.querySelector('[data-top-level="shown"]')).not.toBeNull();
    expect(
      document.querySelector('[data-top-level-tail="shown"]'),
    ).not.toBeNull();
    expect(
      document.querySelector('[data-nested="switch-fallback"]'),
    ).not.toBeNull();
    expect(
      document.querySelector('[data-keyed-show="fallback"]'),
    ).not.toBeNull();
    expect(
      document.querySelector('[data-live-show="fallback"]'),
    ).not.toBeNull();
    expect(
      document.querySelector('[data-keyed-match="fallback"]'),
    ).not.toBeNull();
    expect(document.querySelector('[data-switch="fallback"]')).not.toBeNull();
    expect(root.dataset.componentCleanups).toBe("0");
    expect(root.dataset.forFallbackCleanups).toBe("0");
    expect(root.dataset.boundaryCleanups).toBe("0");
    expect(root.dataset.initialBoundaryCleanups).toBe("1");
    expect(root.dataset.suspenseContentCleanups).toBe("0");
    expect(root.dataset.suspenseFallbackCleanups).toBe("0");
    expect(document.querySelector('[data-suspense="content"]')).toBeNull();
    expect(
      document.querySelector('[data-suspense="fallback"]')?.textContent,
    ).toBe("Loading");
    const firstBoundary = document.querySelector('[data-boundary="healthy"]');
    expect(firstBoundary?.textContent).toBe("Healthy");
    expect(
      document
        .querySelector('[data-boundary-initial="fallback"]')
        ?.textContent.trim(),
    ).toBe("Initial boundary failure");
    expect(document.querySelector('[data-dynamic="aside"]')?.tagName).toBe(
      "ASIDE",
    );
    const spreadButton = document.querySelector('[data-spread-ref="captured"]');
    const spreadInput = document.querySelector('[data-spread-input="present"]');
    expect(spreadButton?.dataset.order).toBe("after");
    expect(spreadButton?.title).toBe("spread-initial");
    expect(spreadButton?.classList.contains("ready")).toBe(true);
    expect(spreadButton?.classList.contains("spread")).toBe(true);
    expect(spreadButton?.classList.contains("pair")).toBe(true);
    expect(spreadButton?.style.color).toBe("green");
    expect(spreadButton?.style.getPropertyValue("--spread-count")).toBe("0");
    expect(spreadInput?.value).toBe("0");
    expect(spreadInput?.checked).toBe(true);
    expect(document.querySelector('[data-action="portal"]')).not.toBeNull();
    expect(document.title).toBe("SolidLil 0");
    expect(document.querySelector('[data-portal-svg="circle"]')).not.toBeNull();
    const shadowHost = document.querySelector('[data-shadow-ref="captured"]');
    expect(shadowHost?.shadowRoot).not.toBeNull();
    expect(
      shadowHost?.shadowRoot?.querySelector('[data-portal-shadow="content"]')
        ?.textContent,
    ).toBe("Shadow 0");

    click("increment");
    expect(root.dataset.count).toBe("1");
    expect(root.classList.contains("counted")).toBe(true);
    expect(input.value).toBe("1");
    expect(document.querySelector("h1").textContent).toBe("Count 1");
    expect(document.title).toBe("SolidLil 1");
    const firstKeyedShow = document.querySelector('[data-keyed-show="1"]');
    const firstLiveShow = document.querySelector('[data-live-show="1"]');
    const firstKeyedMatch = document.querySelector('[data-keyed-match="1"]');
    expect(firstKeyedShow?.textContent).toBe("1");
    expect(firstLiveShow?.textContent).toBe("1");
    expect(firstKeyedMatch?.textContent).toBe("1");
    expect(
      shadowHost?.shadowRoot?.querySelector('[data-portal-shadow="content"]')
        ?.textContent,
    ).toBe("Shadow 1");

    click("switch");
    const retiredSwitchBranch = document.querySelector("[data-component]");
    expect(retiredSwitchBranch?.dataset.component).toBe("after");
    expect(retiredSwitchBranch?.textContent).toBe("First 1 Child");
    expect(
      document.querySelector('[data-component-child="present"]'),
    ).not.toBeNull();
    click("switch");
    expect(document.querySelector("[data-component]")).toBeNull();
    expect(document.querySelector('[data-switch="second"]')).not.toBeNull();
    expect(root.dataset.componentCleanups).toBe("1");
    expect(document.querySelector('[data-nested="owned"]')?.textContent).toBe(
      "Nested 1",
    );
    expect(
      document.querySelector('[data-keyed-match="priority"]'),
    ).not.toBeNull();
    expect(root.dataset.keyedMatchCleanups).toBe("1");
    expect(document.querySelector('[data-live-match="active"]')).toBeNull();
    click("stale-match");
    expect(spreadButton.dataset.staleMatch).toBe("throw");

    const retiredDynamicNode = document.querySelector('[data-dynamic="aside"]');
    click("dynamic");
    const dynamicComponent = document.querySelector(
      '[data-dynamic-component="greeting"]',
    );
    expect(dynamicComponent?.textContent).toBe("Dynamic 1 Child");
    expect(
      document.querySelector('[data-dynamic-child="present"]'),
    ).not.toBeNull();
    expect(root.dataset.dynamicCleanups).toBe("0");
    click("dynamic");
    expect(document.querySelector('[data-dynamic="article"]')?.tagName).toBe(
      "ARTICLE",
    );
    expect(root.dataset.dynamicCleanups).toBe("1");
    expect(
      document.querySelector('[data-dynamic-svg="path"]')?.namespaceURI,
    ).toBe("http://www.w3.org/2000/svg");
    click("dynamic");
    expect(document.querySelector("[data-dynamic]")).toBeNull();
    click("dynamic");
    expect(document.querySelector('[data-dynamic="aside"]')?.tagName).toBe(
      "ASIDE",
    );

    spreadButton.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(root.dataset.spreadClicks).toBe("1");
    expect(spreadButton.dataset.handlerCount).toBe("1");
    expect(spreadInput.value).toBe("1");
    expect(spreadButton.style.getPropertyValue("--spread-count")).toBe("1");
    click("spread-update");
    expect(spreadButton.title).toBe("spread-updated");
    expect(spreadButton.classList.contains("ready")).toBe(false);
    expect(spreadButton.classList.contains("spread")).toBe(false);
    expect(spreadButton.classList.contains("pair")).toBe(false);
    expect(spreadButton.style.color).toBe("purple");
    expect(spreadInput.checked).toBe(false);
    spreadButton.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(root.dataset.spreadClicks).toBe("11");
    expect(spreadButton.dataset.handlerCount).toBe("11");
    expect(spreadInput.value).toBe("11");

    click("portal");
    expect(root.dataset.count).toBe("101");
    const secondKeyedShow = document.querySelector('[data-keyed-show="101"]');
    const secondLiveShow = document.querySelector('[data-live-show="101"]');
    expect(secondKeyedShow?.textContent).toBe("101");
    expect(secondKeyedShow).not.toBe(firstKeyedShow);
    expect(root.dataset.keyedShowCleanups).toBe("1");
    expect(secondLiveShow?.textContent).toBe("101");
    expect(secondLiveShow).toBe(firstLiveShow);
    expect(retiredSwitchBranch?.textContent).toBe("First 1 Child");
    const retiredDynamicText = retiredDynamicNode?.textContent;

    action("native").dispatchEvent(new Event("scroll"));
    expect(root.dataset.count).toBe("111");

    const originalFirst = document.querySelector('[data-row="1"]');
    click("rows");
    const rowElements = [...document.querySelectorAll("[data-row]")];
    expect(rowElements.map((node) => node.textContent)).toEqual([
      "2:0",
      "1:1",
      "3:2",
    ]);
    const keyedIdentityPreserved = rowElements[1] === originalFirst;
    expect(
      keyedIdentityPreserved,
      `${dom.window.__lsxRuntimeKind}: keyed reorder keeps the original row`,
    ).toBe(true);
    expect(root.dataset.rowCleanups).toBe("0");

    click("rows-duplicate");
    const duplicateRows = [...document.querySelectorAll("[data-row]")];
    expect(duplicateRows.map((node) => node.textContent)).toEqual([
      "1:0",
      "1:1",
      "3:2",
    ]);
    const duplicateIdentityPreserved = duplicateRows[1] === originalFirst;
    const duplicateCopy = duplicateRows[0];
    const originalThree = duplicateRows[2];
    expect(
      duplicateIdentityPreserved,
      `${dom.window.__lsxRuntimeKind}: common-suffix duplicate keeps the keyed row`,
    ).toBe(true);
    expect(duplicateCopy).not.toBe(originalFirst);
    expect(root.dataset.rowCleanups).toBe("1");

    click("rows-remove");
    const reducedRows = [...document.querySelectorAll("[data-row]")];
    expect(reducedRows.map((node) => node.textContent)).toEqual(["1:0", "3:1"]);
    expect(reducedRows[0]).toBe(duplicateCopy);
    expect(reducedRows[1]).toBe(originalThree);
    expect(originalFirst.isConnected).toBe(false);
    expect(root.dataset.rowCleanups).toBe("2");

    click("rows-clear");
    expect(document.querySelector('[data-row="empty"]')?.textContent).toBe(
      "Empty",
    );
    expect(root.dataset.rowCleanups).toBe("4");
    expect(root.dataset.forFallbackCleanups).toBe("0");

    click("rows-restore");
    const finalRows = [...document.querySelectorAll("[data-row]")];
    expect(finalRows.map((node) => node.textContent)).toEqual(["3:0", "4:1"]);
    expect(finalRows[0]).not.toBe(originalThree);
    expect(root.dataset.forFallbackCleanups).toBe("1");

    click("indexed");
    expect(
      [...document.querySelectorAll("[data-index]")].map(
        (node) => node.textContent,
      ),
    ).toEqual(["7"]);
    expect(root.dataset.indexedCleanups).toBe("1");

    click("boundary-fail");
    expect(document.querySelector('[data-boundary="healthy"]')).toBeNull();
    expect(action("boundary-reset")?.textContent).toBe("Boundary failure");
    expect(root.dataset.boundaryCleanups).toBe("1");
    click("boundary-reset");
    const resetBoundary = document.querySelector('[data-boundary="healthy"]');
    expect(resetBoundary?.textContent).toBe("Healthy");
    expect(resetBoundary).not.toBe(firstBoundary);
    expect(root.dataset.boundaryCleanups).toBe("1");

    click("suspense-resolve-first");
    await Promise.resolve();
    await Promise.resolve();
    expect(document.querySelector('[data-suspense="content"]')).toBeNull();
    expect(
      document.querySelector('[data-suspense="fallback"]')?.textContent,
    ).toBe("Loading");
    expect(root.dataset.suspenseFallbackCleanups).toBe("0");
    click("suspense-resolve-second");
    await Promise.resolve();
    await Promise.resolve();
    expect(document.querySelector('[data-suspense="fallback"]')).toBeNull();
    expect(
      document.querySelector('[data-suspense="content"]')?.textContent,
    ).toBe("First + Second");
    expect(root.dataset.suspenseContentCleanups).toBe("0");
    expect(root.dataset.suspenseFallbackCleanups).toBe("1");

    click("toggle");
    expect(document.querySelector('[data-state="shown"]')).toBeNull();
    expect(document.querySelector('[data-state="hidden"]')).not.toBeNull();
    expect(document.querySelector('[data-nested="owned"]')).toBeNull();
    expect(
      document.querySelector('[data-nested="outer-fallback"]'),
    ).not.toBeNull();
    expect(document.querySelector('[data-top-level="shown"]')).toBeNull();
    expect(document.querySelector('[data-top-level-tail="shown"]')).toBeNull();
    expect(document.querySelector('[data-top-level="hidden"]')).not.toBeNull();
    expect(root.dataset.nestedCleanups).toBe("1");
    expect(input.checked).toBe(false);
    expect(input.disabled).toBe(true);

    for (let cycle = 0; cycle < 20; cycle += 1) click("toggle");
    expect(document.querySelector('[data-state="hidden"]')).not.toBeNull();
    expect(document.querySelector('[data-top-level="hidden"]')).not.toBeNull();
    expect(root.dataset.nestedCleanups).toBe("11");

    const svg = document.querySelector('[data-namespace="svg"]');
    const circle = document.querySelector('[data-shape="circle"]');
    const math = document.querySelector('[data-namespace="math"]');
    const use = document.querySelector('[data-shape="use"]');
    const xmlText = document.querySelector('[data-xml="language"]');
    const portalCircle = document.querySelector('[data-portal-svg="circle"]');
    expect(svg.namespaceURI).toBe("http://www.w3.org/2000/svg");
    expect(circle.namespaceURI).toBe("http://www.w3.org/2000/svg");
    expect(portalCircle.namespaceURI).toBe("http://www.w3.org/2000/svg");
    expect(math.namespaceURI).toBe("http://www.w3.org/1998/Math/MathML");
    expect(use.getAttributeNS("http://www.w3.org/1999/xlink", "href")).toBe(
      "#hidden",
    );
    expect(
      xmlText.getAttributeNS("http://www.w3.org/XML/1998/namespace", "lang"),
    ).toBe("tr");

    const digest = {
      count: root.dataset.count,
      classes: [...root.classList].sort(),
      input: {
        checked: input.checked,
        disabled: input.disabled,
        required: input.required,
        value: input.value,
      },
      ref: document.querySelector('[data-ref="captured"]') !== null,
      directive: document.querySelector('[data-directive="ready"]') !== null,
      show: document.querySelector('[data-state="hidden"]')?.textContent,
      rows: finalRows.map((node) => node.textContent),
      keyedIdentityPreserved,
      duplicateIdentityPreserved,
      indexed: [...document.querySelectorAll("[data-index]")].map(
        (node) => node.textContent,
      ),
      switch: document.querySelector("[data-switch]")?.dataset.switch,
      componentCleanups: root.dataset.componentCleanups,
      rowCleanups: root.dataset.rowCleanups,
      forFallbackCleanups: root.dataset.forFallbackCleanups,
      indexedCleanups: root.dataset.indexedCleanups,
      boundaryCleanups: root.dataset.boundaryCleanups,
      initialBoundaryCleanups: root.dataset.initialBoundaryCleanups,
      suspenseContentCleanups: root.dataset.suspenseContentCleanups,
      suspenseFallbackCleanups: root.dataset.suspenseFallbackCleanups,
      nestedCleanups: root.dataset.nestedCleanups,
      keyedShowCleanups: root.dataset.keyedShowCleanups,
      keyedMatchCleanups: root.dataset.keyedMatchCleanups,
      dynamic: {
        tag: document.querySelector("[data-dynamic]")?.tagName,
        value: document.querySelector("[data-dynamic]")?.dataset.dynamic,
        retiredText: retiredDynamicText,
        cleanups: root.dataset.dynamicCleanups,
      },
      spread: {
        clicks: root.dataset.spreadClicks,
        handlerCount: spreadButton.dataset.handlerCount,
        order: spreadButton.dataset.order,
        title: spreadButton.title,
        classes: [...spreadButton.classList].sort(),
        color: spreadButton.style.color,
        custom: spreadButton.style.getPropertyValue("--spread-count"),
        inputValue: spreadInput.value,
        inputChecked: spreadInput.checked,
        staleMatch: spreadButton.dataset.staleMatch,
      },
      portal: document.querySelector('[data-action="portal"]')?.textContent,
      portalHead: document.title,
      portalSvg: portalCircle.namespaceURI,
      portalShadow: {
        ref: shadowHost?.dataset.shadowRef,
        text: shadowHost?.shadowRoot?.querySelector(
          '[data-portal-shadow="content"]',
        )?.textContent,
      },
      xlink: use.getAttributeNS("http://www.w3.org/1999/xlink", "href"),
      xml: xmlText.getAttributeNS(
        "http://www.w3.org/XML/1998/namespace",
        "lang",
      ),
      namespaces: [svg.namespaceURI, circle.namespaceURI, math.namespaceURI],
    };

    const staleButton = action("increment");
    dom.window.__disposeLsx();
    dom.window.__disposeLsx();
    expect(document.querySelector("#app").childNodes).toHaveLength(0);
    expect(document.querySelector("#portal-target").childNodes).toHaveLength(0);
    expect(
      document.querySelector("#svg-portal-target").childNodes,
    ).toHaveLength(0);
    expect(
      document.querySelector("#shadow-portal-target").childNodes,
    ).toHaveLength(0);
    expect(document.title).toBe("");
    staleButton.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    const staleSpreadCount = spreadButton.dataset.handlerCount;
    spreadButton.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(spreadButton.dataset.handlerCount).toBe(staleSpreadCount);
    expect(document.querySelector("#app").childNodes).toHaveLength(0);
    if (dom.window.__lsxDiagnostics) {
      const slots = dom.window.__lsxDiagnostics();
      expect(slots.freeOwners).toBe(slots.owners);
      expect(slots.freeEffects).toBe(slots.effects);
      expect(slots.pendingEffects).toBe(0);
    }
    const boundaryDiagnostics = dom.window.__lsxBoundaryDiagnostics?.();
    expect(boundaryDiagnostics).toEqual({
      boundaryCleanups: 2,
      initialBoundaryCleanups: 1,
      suspenseContentCleanups: 1,
      suspenseFallbackCleanups: 1,
    });
    digest.boundaryUnmountCleanups = boundaryDiagnostics;
    digest.unmounted = document.querySelector("#app").childNodes.length === 0;
    return digest;
  } finally {
    dom.window.close();
  }
}

function exercisePendingUnmount(dom) {
  const { document } = dom.window;
  try {
    expect(document.querySelector('[data-suspense="content"]')).toBeNull();
    expect(
      document.querySelector('[data-suspense="fallback"]')?.textContent,
    ).toBe("Loading");
    dom.window.__disposeLsx();
    dom.window.__disposeLsx();
    expect(document.querySelector("#app").childNodes).toHaveLength(0);
    const cleanupDigest = dom.window.__lsxBoundaryDiagnostics?.();
    expect(cleanupDigest).toEqual({
      boundaryCleanups: 1,
      initialBoundaryCleanups: 1,
      suspenseContentCleanups: 1,
      suspenseFallbackCleanups: 1,
    });
    if (dom.window.__lsxDiagnostics) {
      const slots = dom.window.__lsxDiagnostics();
      expect(slots.freeOwners).toBe(slots.owners);
      expect(slots.freeEffects).toBe(slots.effects);
      expect(slots.pendingEffects).toBe(0);
    }
    return cleanupDigest;
  } finally {
    dom.window.close();
  }
}

describe("SolidLil LSX integrated runtime", () => {
  test("matches official Solid JSX through updates and unmount", async () => {
    const solidDigest = await exerciseInBrowser(await mount("solid"), exercise);
    const solidlilDigest = await exerciseInBrowser(
      await mount("solidlil"),
      exercise,
    );
    expect(solidlilDigest).toEqual(solidDigest);
  });

  test("releases a still-pending Suspense subtree on unmount", async () => {
    const solidDigest = await exerciseInBrowser(
      await mount("solid"),
      exercisePendingUnmount,
    );
    const solidlilDigest = await exerciseInBrowser(
      await mount("solidlil"),
      exercisePendingUnmount,
    );
    expect(solidlilDigest).toEqual(solidDigest);
  });

  test.runIf(process.env.SOLIDLIL_TEST_BUILT_LSX === "1")(
    "matches official Solid after production bundling and minification",
    async () => {
      const solidDigest = await exerciseInBrowser(
        await mountBuilt("solid"),
        exercise,
      );
      const solidlilDigest = await exerciseInBrowser(
        await mountBuilt("solidlil"),
        exercise,
      );
      expect(solidlilDigest).toEqual(solidDigest);
    },
  );
});
