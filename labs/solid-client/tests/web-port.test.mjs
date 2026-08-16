import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { afterAll, describe, expect as vitestExpect, test } from "vitest";
import { build } from "vite";
import { chromium } from "../../../benchmarks/browser/playwright-runtime.mjs";
import { compilerPath, projectRoot } from "../tooling/compiler-path.mjs";

const compiledWebPorts = new Map();
let hostBundle;
let browser;

const expect = (...arguments_) => vitestExpect(...arguments_);
const markup =
  '<!doctype html><html><body><main id="app"></main><aside id="portal-target"></aside><svg id="svg-portal-target"></svg></body></html>';

// Serialized browser callbacks resolve this name from the Chromium global.
function mountWebPort(...arguments_) {
  return globalThis.mountWebPort(...arguments_);
}

async function browserInstance() {
  browser ??= await chromium.launch({ headless: true });
  return browser;
}

afterAll(async () => {
  await browser?.close();
});

function compileWebPort(mode = "maximum") {
  if (compiledWebPorts.has(mode)) return compiledWebPorts.get(mode);
  const output = resolve(
    tmpdir(),
    `lilscript-web-test-${process.pid}-${mode}.js`,
  );
  const args = [
    resolve(projectRoot, "tests/lil/web-behavior.lil"),
    "--target",
    "js",
    "-o",
    output,
  ];
  if (mode === "none") {
    args.push(
      "--config",
      resolve(projectRoot, "compatibility/config/none.toml"),
    );
  }
  const result = spawnSync(compilerPath(), args, {
    encoding: "utf8",
    env: process.env,
  });
  if (result.status !== 0) throw new Error(result.stderr);
  const compiled = readFileSync(output, "utf8");
  compiledWebPorts.set(mode, compiled);
  return compiled;
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
        name: "SolidLilWebHost",
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

async function runBrowserTest(callback, mode) {
  const context = await (await browserInstance()).newContext();
  const page = await context.newPage();
  try {
    await page.setContent(markup, { waitUntil: "load" });
    await page.evaluate((expectedMode) => {
      globalThis.close = () => {};
      const format = (value) => {
        if (value instanceof globalThis.Node)
          return value.outerHTML ?? value.nodeName;
        try {
          return JSON.stringify(value);
        } catch {
          return String(value);
        }
      };
      const equal = (left, right) => {
        if (Object.is(left, right)) return true;
        if (Array.isArray(left) && Array.isArray(right))
          return (
            left.length === right.length &&
            left.every((value, index) => equal(value, right[index]))
          );
        if (
          left &&
          right &&
          Object.getPrototypeOf(left) === Object.prototype &&
          Object.getPrototypeOf(right) === Object.prototype
        ) {
          const leftKeys = Object.keys(left);
          const rightKeys = Object.keys(right);
          return (
            leftKeys.length === rightKeys.length &&
            leftKeys.every(
              (key) =>
                Object.hasOwn(right, key) && equal(left[key], right[key]),
            )
          );
        }
        return false;
      };
      globalThis.expect = (actual, message = "browser expectation") => {
        const matchers = (inverted) => {
          const verify = (matches, expected, matcher) => {
            if (inverted ? matches : !matches) {
              throw new Error(
                `${message}: expected ${format(actual)} ${inverted ? "not " : ""}${matcher} ${format(expected)}`,
              );
            }
          };
          const api = {
            toBe: (expected) =>
              verify(Object.is(actual, expected), expected, "to be"),
            toBeInstanceOf: (expected) =>
              verify(
                actual instanceof expected,
                expected?.name,
                "to be an instance of",
              ),
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
      globalThis.mountWebPort = (requestedMode = "maximum") => {
        if (requestedMode !== expectedMode)
          throw new Error(
            `expected ${expectedMode} build, received ${requestedMode}`,
          );
        return {
          action: (name) => document.querySelector(`[data-action="${name}"]`),
          click: (node) =>
            node.dispatchEvent(
              new globalThis.MouseEvent("click", {
                bubbles: true,
                cancelable: true,
                composed: true,
              }),
            ),
          document,
          dom: { window: globalThis },
        };
      };
    }, mode);
    await page.evaluate(
      ({ host, mode, source }) =>
        (0, eval)(
          `${host}\n${source}\n//# sourceURL=web-port-${mode}-playwright.js`,
        ),
      { host: await compileHostBundle(), mode, source: compileWebPort(mode) },
    );
    await page.evaluate(
      async (source) => (0, eval)(`(${source})`)(),
      callback.toString(),
    );
  } finally {
    await context.close();
  }
}

function browserTest(name, callback, mode = "maximum") {
  test(name, () => runBrowserTest(callback, mode));
}

describe("LilScript web runtime", () => {
  browserTest(
    "replaces conditional branches and disposes owned listeners",
    () => {
      const { action, click, document, dom } = mountWebPort();
      try {
        const count = () =>
          document.querySelector('[data-value="clicks"]').textContent;

        expect(document.querySelector('[data-state="hidden"]')).not.toBeNull();
        expect(document.querySelector('[data-state="shown"]')).toBeNull();
        click(action("show"));

        const firstPanel = document.querySelector('[data-state="shown"]');
        const removedButton = action("inside");
        expect(firstPanel.parentElement.dataset.test).toBe("dynamic-region");
        click(removedButton);
        expect(count()).toBe("1");

        click(action("hide"));
        expect(firstPanel.isConnected).toBe(false);
        click(removedButton);
        expect(count()).toBe("1");

        click(action("show"));
        const secondPanel = document.querySelector('[data-state="shown"]');
        expect(secondPanel).not.toBe(firstPanel);
        const secondRemovedButton = action("inside");
        click(secondRemovedButton);
        expect(count()).toBe("2");
        click(action("hide"));
        click(secondRemovedButton);
        expect(count()).toBe("2");

        const removedShow = action("show");
        click(action("dispose"));
        expect(document.querySelector("#app").childNodes).toHaveLength(0);
        click(removedShow);
        expect(document.querySelector('[data-state="shown"]')).toBeNull();
      } finally {
        dom.window.close();
      }
    },
  );

  browserTest(
    "reconciles keyed and positional lists with owned fallbacks",
    () => {
      const { action, click, document, dom } = mountWebPort();
      try {
        const rowCount = () =>
          document.querySelector('[data-value="row-clicks"]').textContent;
        const rows = () => [...document.querySelectorAll("[data-item]")];

        const initialRows = rows();
        click(action("rotate"));
        expect(rows()).toEqual([
          initialRows[2],
          initialRows[0],
          initialRows[1],
        ]);
        expect(rows().map((node) => node.textContent)).toEqual([
          "3:0",
          "1:1",
          "2:2",
        ]);
        const removedRow = initialRows[0];
        click(removedRow);
        expect(rowCount()).toBe("1");
        click(action("remove-item"));
        expect(rows()).toEqual([initialRows[2], initialRows[1]]);
        expect(removedRow.isConnected).toBe(false);
        click(removedRow);
        expect(rowCount()).toBe("1");
        click(action("clear-items"));
        expect(rows()).toHaveLength(0);
        expect(
          document.querySelector('[data-list-fallback="empty"]'),
        ).not.toBeNull();
        click(action("restore-items"));
        expect(rows()).toHaveLength(3);
        expect(rows()[0]).not.toBe(initialRows[0]);

        const indexedRows = () => [
          ...document.querySelectorAll("[data-position]"),
        ];
        const initialIndexed = indexedRows();
        click(action("update-index"));
        expect(indexedRows()).toHaveLength(2);
        expect(indexedRows()[0]).toBe(initialIndexed[0]);
        expect(indexedRows()[1]).toBe(initialIndexed[1]);
        expect(indexedRows().map((node) => node.textContent)).toEqual([
          "4",
          "5",
        ]);
        click(action("shrink-index"));
        expect(indexedRows()).toHaveLength(1);
        expect(indexedRows()[0]).toBe(initialIndexed[0]);
        expect(initialIndexed[0].textContent).toBe("7");
        expect(initialIndexed[1].isConnected).toBe(false);
        click(action("clear-index"));
        expect(indexedRows()).toHaveLength(0);
        expect(
          document.querySelector('[data-index-fallback="empty"]'),
        ).not.toBeNull();
        click(action("restore-index"));
        expect(indexedRows()).toHaveLength(2);
        expect(indexedRows()[0]).not.toBe(initialIndexed[0]);
        expect(indexedRows().map((node) => node.textContent)).toEqual([
          "8",
          "9",
        ]);
      } finally {
        dom.window.close();
      }
    },
  );

  browserTest(
    "updates typed attributes, properties, classes, and styles",
    () => {
      const { action, click, document, dom } = mountWebPort();
      try {
        const field = document.querySelector('[data-element="field"]');
        expect(field.title).toBe("initial");
        expect(field.value).toBe("alpha");
        expect(field.disabled).toBe(false);
        expect(field.checked).toBe(false);
        expect(field.required).toBe(true);
        expect(field.readOnly).toBe(true);
        expect(field.placeholder).toBe("Static placeholder");
        expect(field.className).toBe("base");
        expect(field.style.backgroundColor).toBe("white");
        expect(field.dataset.directive).toBe("applied");
        click(action("activate-field"));
        expect(field.title).toBe("updated");
        expect(field.value).toBe("beta");
        expect(field.disabled).toBe(true);
        expect(field.checked).toBe(true);
        expect(field.classList.contains("first")).toBe(true);
        expect(field.classList.contains("second")).toBe(true);
        expect(field.style.color).toBe("red");
        click(action("reset-field"));
        expect(field.disabled).toBe(false);
        expect(field.checked).toBe(false);
        expect(field.className).toBe("base");
        expect(field.style.color).toBe("");
      } finally {
        dom.window.close();
      }
    },
  );

  browserTest(
    "delegates typed events with browser propagation semantics",
    () => {
      const { action, click, document, dom } = mountWebPort();
      try {
        const trace = () =>
          document.querySelector('[data-value="delegated-events"]').textContent;
        expect(click(action("delegated-child").firstChild)).toBe(false);
        expect(trace()).toBe("payload:click:false:true|parent:true:true");
        expect(click(action("delegated-stop"))).toBe(true);
        expect(trace()).toBe("payload:click:false:true|parent:true:true|stop");
        expect(click(action("delegated-disabled"))).toBe(true);
        expect(trace()).toBe(
          "payload:click:false:true|parent:true:true|stop|parent:false:true",
        );
      } finally {
        dom.window.close();
      }
    },
  );

  browserTest(
    "replaces dynamic intrinsic elements and typed components",
    () => {
      const { action, click, document, dom } = mountWebPort();
      try {
        const staticSvg = document.querySelector('[data-namespace="svg"]');
        const staticCircle = document.querySelector(
          '[data-namespace-child="circle"]',
        );
        const staticMath = document.querySelector('[data-namespace="math"]');
        expect(staticSvg).toBeInstanceOf(dom.window.SVGSVGElement);
        expect(staticCircle).toBeInstanceOf(dom.window.SVGElement);
        expect(staticMath.namespaceURI).toBe(
          "http://www.w3.org/1998/Math/MathML",
        );
        expect(staticMath.firstChild.namespaceURI).toBe(
          "http://www.w3.org/1998/Math/MathML",
        );

        const dynamicNode = () =>
          document.querySelector('[data-dynamic-node="active"]');
        expect(dynamicNode()).toBeNull();
        click(action("dynamic-div"));
        const dynamicDiv = dynamicNode();
        expect(dynamicDiv.tagName).toBe("DIV");
        expect(dynamicDiv.dataset.name).toBe("Smith");
        expect(dynamicDiv.textContent).toBe("Hi Smith");
        click(action("dynamic-name"));
        expect(dynamicNode()).toBe(dynamicDiv);
        expect(dynamicDiv.dataset.name).toBe("Sunny");
        expect(dynamicDiv.textContent).toBe("Hi Sunny");
        click(action("dynamic-span"));
        const dynamicSpan = dynamicNode();
        expect(dynamicSpan.tagName).toBe("SPAN");
        expect(dynamicSpan).not.toBe(dynamicDiv);
        expect(dynamicDiv.isConnected).toBe(false);
        expect(dynamicSpan.dataset.name).toBe("Sunny");
        click(action("dynamic-svg"));
        const dynamicSvg = dynamicNode();
        expect(dynamicSvg).toBeInstanceOf(dom.window.SVGSVGElement);
        expect(dynamicSvg.namespaceURI).toBe("http://www.w3.org/2000/svg");
        click(action("dynamic-path"));
        const dynamicPath = dynamicNode();
        expect(dynamicPath).toBeInstanceOf(dom.window.SVGElement);
        expect(dynamicPath.namespaceURI).toBe("http://www.w3.org/2000/svg");
        expect(dynamicSvg.isConnected).toBe(false);
        click(action("dynamic-clear"));
        expect(dynamicNode()).toBeNull();
        expect(dynamicPath.isConnected).toBe(false);

        const component = () =>
          document.querySelector("[data-dynamic-component]");
        const componentA = component();
        expect(componentA.dataset.dynamicComponent).toBe("a");
        expect(componentA.textContent).toBe("A One");
        click(action("component-name"));
        expect(component()).toBe(componentA);
        expect(componentA.textContent).toBe("A Two");
        click(action("component-b"));
        const componentB = component();
        expect(componentB.dataset.dynamicComponent).toBe("b");
        expect(componentB.textContent).toBe("B Two");
        expect(componentB).not.toBe(componentA);
        expect(componentA.isConnected).toBe(false);

        const dynamicChoiceRoot = document.querySelector(
          '[data-component-root="union"]',
        );
        expect(dynamicChoiceRoot.childElementCount).toBe(0);
        click(action("dynamic-choice-a"));
        const choiceA = dynamicChoiceRoot.querySelector(
          '[data-dynamic-component="a"]',
        );
        expect(choiceA.textContent).toBe("A Two");
        click(action("dynamic-choice-b"));
        const choiceB = dynamicChoiceRoot.querySelector(
          '[data-dynamic-component="b"]',
        );
        expect(choiceB.textContent).toBe("B Two");
        expect(choiceA.isConnected).toBe(false);
        click(action("dynamic-choice-h1"));
        const choiceH1 = dynamicChoiceRoot.querySelector(
          '[data-dynamic-choice="intrinsic"]',
        );
        expect(choiceH1.tagName).toBe("H1");
        expect(choiceH1.id).toBe("Two");
        click(action("dynamic-choice-name"));
        expect(choiceH1.id).toBe("Three");
        click(action("dynamic-choice-svg"));
        expect(
          dynamicChoiceRoot.querySelector('[data-dynamic-choice="intrinsic"]'),
        ).toBeInstanceOf(dom.window.SVGSVGElement);
        click(action("dynamic-choice-path"));
        expect(
          dynamicChoiceRoot.querySelector('[data-dynamic-choice="intrinsic"]'),
        ).toBeInstanceOf(dom.window.SVGElement);
        click(action("dynamic-choice-clear"));
        expect(dynamicChoiceRoot.childElementCount).toBe(0);
      } finally {
        dom.window.close();
      }
    },
  );

  browserTest(
    "renders owned HTML and SVG portals with synthetic events",
    () => {
      const { action, click, document, dom } = mountWebPort();
      try {
        const htmlPortal = document.querySelector('[data-portal="html"]');
        const svgPortal = document.querySelector('[data-portal="svg"]');
        const portalText = () =>
          document.querySelector('[data-portal-value="text"]').textContent;
        const portalTrace = () =>
          document.querySelector('[data-portal-value="trace"]').textContent;
        expect(document.querySelector("#app").contains(htmlPortal)).toBe(false);
        expect(htmlPortal.parentElement.id).toBe("portal-target");
        expect(portalText()).toBe("Portal One");
        expect(svgPortal).toBeInstanceOf(dom.window.SVGGElement);
        expect(svgPortal.parentElement.id).toBe("svg-portal-target");
        click(action("portal-update"));
        expect(portalText()).toBe("Portal Two");
        click(action("portal-child"));
        expect(portalTrace()).toBe("child|host");
        click(action("clear-delegated"));
        click(action("portal-child"));
        expect(portalTrace()).toBe("child|host");
        click(action("dispose"));
        expect(
          document.querySelector("#portal-target").childNodes,
        ).toHaveLength(0);
        expect(
          document.querySelector("#svg-portal-target").childNodes,
        ).toHaveLength(0);
      } finally {
        dom.window.close();
      }
    },
  );

  browserTest(
    "dispatches component and intrinsic unions without optional optimizations",
    () => {
      const { action, click, document, dom } = mountWebPort("none");
      try {
        const root = document.querySelector('[data-component-root="union"]');
        expect(root.childElementCount).toBe(0);
        click(action("dynamic-choice-a"));
        expect(
          root.querySelector('[data-dynamic-component="a"]').textContent,
        ).toBe("A One");
        click(action("dynamic-choice-h1"));
        const heading = root.querySelector('[data-dynamic-choice="intrinsic"]');
        expect(heading.tagName).toBe("H1");
        expect(heading.id).toBe("One");
        click(action("dynamic-choice-clear"));
        expect(root.childElementCount).toBe(0);
      } finally {
        dom.window.close();
      }
    },
    "none",
  );
});
