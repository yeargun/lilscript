import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { build } from "vite";
import { chromium } from "../../../benchmarks/browser/playwright-runtime.mjs";
import {
  canonicalCodecProvenance,
  canonicalCodecSizes,
} from "../../../benchmarks/codec-contract.mjs";
import { selectSolidLilDistribution } from "./distribution-selection.mjs";
import { root } from "./project.mjs";

const generated = resolve(root, "artifacts/generated");
const buildModesPath = resolve(root, "artifacts/build-modes.json");
const buildModesBytes = readFileSync(buildModesPath);
const buildModes = JSON.parse(buildModesBytes);
const surfaces = {
  full: {
    entries: {
      solid: resolve(root, "api/solid-web.js"),
      solidlil: resolve(root, "api/solidlil-web.js"),
    },
    outputs: {
      solid: resolve(generated, "solid-web.js"),
      solidlil: resolve(generated, "solidlil-web.js"),
    },
  },
  client: {
    entries: {
      solid: resolve(root, "api/solid-web-client.js"),
      solidlil: resolve(root, "api/solidlil-web-client.js"),
    },
    outputs: {
      solid: resolve(generated, "solid-web-client.js"),
      solidlil: resolve(generated, "solidlil-web-client.js"),
    },
  },
};

mkdirSync(generated, { recursive: true });

async function bundle(entry, output, { clientOnly = false } = {}) {
  const result = await build({
    configFile: false,
    root,
    logLevel: "error",
    resolve: {
      conditions: ["browser", "module", "import", "default"],
    },
    define: clientOnly
      ? { "import.meta.env.SOLIDLIL_CLIENT_ONLY": "true" }
      : undefined,
    build: {
      target: "es2022",
      minify: "oxc",
      write: false,
      lib: { entry, formats: ["es"], fileName: "bundle" },
      rolldownOptions: { output: { codeSplitting: false } },
    },
  });
  const buildOutputs = Array.isArray(result)
    ? result.flatMap((item) => item.output)
    : result.output;
  const chunks = buildOutputs.filter((item) => item.type === "chunk");
  assert.equal(chunks.length, 1, `${entry} should emit one JavaScript chunk`);
  const code = `${chunks[0].code.trim()}\n`;
  writeFileSync(output, code);
  return code;
}

function size(code) {
  const measured = canonicalCodecSizes(
    code,
    "SolidLil Web surface verification",
  );
  return {
    schemaVersion: 2,
    brotli11: measured.brotli,
    gzip9: measured.gzip,
    raw: measured.raw,
  };
}

function serialize(value) {
  if (value instanceof Set) return [...value].sort();
  return value;
}

function capture(callback) {
  try {
    const value = callback();
    return { value: value === undefined ? "undefined" : String(value) };
  } catch (error) {
    return { error: error?.constructor?.name, message: error?.message };
  }
}

function unwrapReactive(value) {
  let depth = 0;
  while (typeof value === "function" && depth++ < 30) value = value();
  if (Array.isArray(value)) return value.map(unwrapReactive);
  return value;
}

async function behaviorDigest(module, { includeCompatibility = true } = {}) {
  document.head.textContent = "";
  document.body.textContent = "";
  delete globalThis._$HY;
  const covered = new Set();
  const api = new Proxy(module, {
    get(target, property, receiver) {
      if (typeof property === "string") covered.add(property);
      return Reflect.get(target, property, receiver);
    },
  });

  const constants = {
    Aliases: Object.entries(api.Aliases).sort(),
    ChildProperties: serialize(api.ChildProperties),
    DOMElements: serialize(api.DOMElements),
    DelegatedEvents: serialize(api.DelegatedEvents),
    Properties: serialize(api.Properties),
    SVGElements: serialize(api.SVGElements),
    SVGNamespace: Object.entries(api.SVGNamespace).sort(),
    isDev: api.isDev,
    isServer: api.isServer,
  };
  if (includeCompatibility)
    constants.RequestContext = typeof api.RequestContext;

  const element = document.createElement("button");
  api.setAttribute(element, "data-value", "one");
  api.setAttribute(element, "data-remove", "yes");
  api.setAttribute(element, "data-remove", null);
  api.setBoolAttribute(element, "disabled", true);
  api.setBoolAttribute(element, "hidden", false);
  api.setProperty(element, "value", "property");
  api.className(element, "alpha");
  const classes = api.classList(element, { alpha: true, "beta gamma": true });
  api.classList(element, { alpha: false, delta: true }, classes);
  const styles = api.style(element, {
    color: "red",
    "--token": "first",
  });
  api.style(element, { color: "blue" }, styles);
  api.setStyleProperty(element, "padding-left", "2px");
  api.setStyleProperty(element, "padding-left", null);
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  api.setAttributeNS(
    svg,
    "http://www.w3.org/1999/xlink",
    "xlink:href",
    "#target",
  );

  const assigned = document.createElement("input");
  let refName = "";
  api.assign(assigned, {
    "attr:data-assigned": "yes",
    "bool:required": true,
    class: "assigned",
    ref: (node) => {
      refName = node.localName;
    },
    value: "42",
  });

  const spreadNode = document.createElement("div");
  const spreadState = api.spread(spreadNode, {
    children: "spread child",
    classList: { spread: true },
    style: { color: "purple" },
    title: "spread title",
  });

  let directEvents = 0;
  let dataEvents = "";
  let delegatedEvents = "";
  api.addEventListener(element, "custom", () => directEvents++, false);
  api.addEventListener(
    element,
    "dataevent",
    [
      (data, event) => {
        dataEvents = `${data}:${event.type}`;
      },
      "payload",
    ],
    false,
  );
  api.delegateEvents(["click"]);
  api.addEventListener(
    element,
    "click",
    [
      (data, event) => {
        delegatedEvents = `${data}:${event.currentTarget.localName}`;
      },
      "delegated",
    ],
    true,
  );
  document.body.append(element);
  element.dispatchEvent(new Event("custom", { bubbles: true }));
  element.dispatchEvent(new Event("dataevent", { bubbles: true }));
  element.dispatchEvent(new Event("click", { bubbles: true }));
  api.clearDelegatedEvents();

  const dynamic = { source: () => "dynamic value" };
  api.dynamicProperty(dynamic, "source");
  const used = api.use(
    (node, suffix) => `${node.localName}:${suffix}`,
    element,
    "used",
  );

  const makeTemplate = api.template(
    "<p class=template>hello</p>",
    false,
    false,
  );
  const firstTemplate = makeTemplate();
  const secondTemplate = makeTemplate.cloneNode();

  const insertion = document.createElement("div");
  api.insert(insertion, "first");
  api.insert(insertion, ["a", document.createElement("br"), 2]);
  api.insert(insertion, () => "function child");

  const rendered = document.createElement("div");
  const renderedChild = document.createElement("strong");
  renderedChild.textContent = "rendered";
  const disposeRender = api.render(() => renderedChild, rendered);
  const renderBeforeDispose = rendered.innerHTML;
  disposeRender();

  const memoValue = api.memo(() => 9)();
  let effectRuns = 0;
  api.effect(() => effectRuns++);
  const untracked = api.untrack(() => "untracked");
  const ownerOutside = api.getOwner();
  const merged = api.mergeProps({ a: 1, shared: "old" }, { shared: "new" });
  const component = api.createComponent((props) => props.value + 1, {
    value: 6,
  });
  const controlHost = document.createElement("div");
  let boundaryControl;
  let suspenseControl;
  let suspenseListControl;
  const disposeControls = api.render(() => {
    boundaryControl = api.ErrorBoundary({
      children: "safe",
      fallback: "failed",
    });
    suspenseControl = api.Suspense({
      children: "ready",
      fallback: "loading",
    });
    suspenseListControl = api.SuspenseList({
      children: "listed",
      revealOrder: "forwards",
    });
    return "";
  }, controlHost);
  const controls = {
    For: api.For({ each: [2, 3], children: (item, index) => item + index() })(),
    Index: api.Index({
      each: [4, 5],
      children: (item, index) => item() + index,
    })(),
    Match: api.Match({ when: true, children: "match" }),
    Show: api.Show({ when: "value", children: (value) => value() })(),
    Switch: api.Switch({
      children: [
        api.Match({ when: false, children: "no" }),
        api.Match({ when: true, children: "yes" }),
      ],
      fallback: "fallback",
    })(),
    ErrorBoundary: unwrapReactive(boundaryControl),
    Suspense: unwrapReactive(suspenseControl),
    SuspenseList: unwrapReactive(suspenseListControl),
  };
  disposeControls();

  const dynamicElement = api.createDynamic(() => "section", {
    children: "dynamic element",
    id: "created-dynamic",
  })();
  const DynamicElement = api.Dynamic({
    component: "aside",
    children: "Dynamic element",
    id: "dynamic-component",
  })();

  const portalHost = document.createElement("div");
  const portalMount = document.createElement("div");
  document.body.append(portalHost, portalMount);
  const portalChild = document.createElement("em");
  portalChild.textContent = "portal";
  const disposePortal = api.render(
    () => api.Portal({ mount: portalMount, children: portalChild }),
    portalHost,
  );
  const portalBeforeDispose = portalMount.textContent;
  disposePortal();

  const propAliases = [
    api.getPropAlias("readonly", "INPUT"),
    api.getPropAlias("readonly", "DIV"),
  ];

  let hydration;
  if (includeCompatibility) {
    const hydrationValues = {
      Hydration: api.Hydration({ children: "hydration" }),
      NoHydration: api.NoHydration({ children: "no hydration" }),
      getHydrationKey: capture(() => api.getHydrationKey()),
      getNextElement: api.getNextElement(() =>
        document.createElement("article"),
      ).localName,
      getNextMarker: api
        .getNextMarker(document.createComment("start"))
        .map((value) =>
          Array.isArray(value) ? value.length : value?.nodeType,
        ),
      getNextMatch: api.getNextMatch(document.createElement("span"), "span")
        ?.localName,
    };
    api.runHydrationEvents();

    globalThis._$HY = { done: true };
    const hydrateHost = document.createElement("div");
    const hydratedNode = document.createElement("i");
    hydratedNode.textContent = "hydrated";
    const disposeHydrate = api.hydrate(() => hydratedNode, hydrateHost);
    const hydrated = hydrateHost.innerHTML;
    disposeHydrate();
    delete globalThis._$HY;
    hydration = { ...hydrationValues, hydrated };
  }

  const inner = document.createElement("div");
  api.innerHTML(inner, "<b>inner</b>");

  let browserErrors;
  let ssr;
  if (includeCompatibility) {
    browserErrors = [];
    const previousConsoleError = console.error;
    console.error = (error) =>
      browserErrors.push(
        (error?.message ?? String(error)).replace(
          /^[^ ]+ is not supported/,
          "API is not supported",
        ),
      );
    const ssrValues = {
      Assets: api.Assets(),
      HydrationScript: api.HydrationScript(),
      escape: api.escape("<unsafe>"),
      generateHydrationScript: api.generateHydrationScript(),
      getAssets: api.getAssets(),
      getRequestEvent: api.getRequestEvent(),
      renderToStream: api.renderToStream(() => "stream"),
      renderToString: api.renderToString(() => "string"),
      renderToStringAsync: api.renderToStringAsync(() => "async"),
      resolveSSRNode: api.resolveSSRNode("node"),
      ssr: api.ssr(["a"], "b"),
      ssrAttribute: api.ssrAttribute("title", "value"),
      ssrClassList: api.ssrClassList({ active: true }),
      ssrElement: api.ssrElement("div", {}, "child", false),
      ssrHydrationKey: api.ssrHydrationKey(),
      ssrSpread: api.ssrSpread({ title: "value" }),
      ssrStyle: api.ssrStyle({ color: "red" }),
      useAssets: api.useAssets(),
    };
    console.error = previousConsoleError;
    ssr = Object.fromEntries(
      Object.entries(ssrValues).map(([key, value]) => [
        key,
        value === undefined ? "undefined" : String(value),
      ]),
    );
  }

  const expected = Object.keys(module).sort();
  if (JSON.stringify([...covered].sort()) !== JSON.stringify(expected)) {
    throw new Error(
      "every verified Solid Web export needs executable coverage",
    );
  }

  return {
    assigned: {
      attributes: [...assigned.attributes]
        .map((attribute) => [attribute.name, attribute.value])
        .sort(),
      className: assigned.className,
      refName,
      required: assigned.required,
      value: assigned.value,
    },
    ...(includeCompatibility ? { browserErrors } : {}),
    component,
    constants,
    controls,
    dynamic: {
      component: DynamicElement.outerHTML,
      created: dynamicElement.outerHTML,
      property: dynamic.source,
      used,
    },
    element: {
      attributes: [...element.attributes]
        .map((attribute) => [attribute.name, attribute.value])
        .sort(),
      className: element.className,
      style: element.style.cssText,
      value: element.value,
      xlink: svg.getAttributeNS("http://www.w3.org/1999/xlink", "href"),
    },
    events: { dataEvents, delegatedEvents, directEvents },
    ...(includeCompatibility ? { hydration } : {}),
    inner: inner.innerHTML,
    insertion: insertion.innerHTML,
    memoValue,
    merged: { a: merged.a, shared: merged.shared },
    ownerOutside: ownerOutside == null ? null : "owner",
    portal: { afterDispose: portalMount.textContent, portalBeforeDispose },
    propAliases,
    reactivity: { effectRuns, untracked },
    render: { afterDispose: rendered.innerHTML, renderBeforeDispose },
    spread: {
      html: spreadNode.outerHTML,
      keys: Object.keys(spreadState).sort(),
    },
    ...(includeCompatibility ? { ssr } : {}),
    template: [firstTemplate.outerHTML, secondTemplate.outerHTML],
  };
}

async function browserBehaviorDigest(browser, code, includeCompatibility) {
  const context = await browser.newContext({ serviceWorkers: "block" });
  const page = await context.newPage();
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  try {
    await page.setContent(
      "<!doctype html><html><head></head><body></body></html>",
      {
        waitUntil: "load",
      },
    );
    const result = await page.evaluate(
      async ({ code, includeCompatibility, sources }) => {
        const createDigest = (0, eval)(
          `(() => { const serialize = ${sources.serialize}; const capture = ${sources.capture}; const unwrapReactive = ${sources.unwrapReactive}; return ${sources.behaviorDigest}; })()`,
        );
        const url = globalThis.URL.createObjectURL(
          new globalThis.Blob([code], { type: "text/javascript" }),
        );
        try {
          const module = await import(url);
          return {
            digest: await createDigest(module, { includeCompatibility }),
            exports: Object.keys(module).sort(),
          };
        } finally {
          globalThis.URL.revokeObjectURL(url);
        }
      },
      {
        code,
        includeCompatibility,
        sources: {
          behaviorDigest: behaviorDigest.toString(),
          capture: capture.toString(),
          serialize: serialize.toString(),
          unwrapReactive: unwrapReactive.toString(),
        },
      },
    );
    assert.deepEqual(pageErrors, [], "Solid Web browser errors");
    return result;
  } finally {
    await context.close();
  }
}

async function verifySurface(browser, name, surface, includeCompatibility) {
  const clientOnly = name === "client";
  const solidCode = await bundle(surface.entries.solid, surface.outputs.solid, {
    clientOnly,
  });
  const selected = await selectSolidLilDistribution({
    clientOnly,
    entry: surface.entries.solidlil,
    output: surface.outputs.solidlil,
    target: `web-${name}`,
  });
  const code = { solid: solidCode, solidlil: selected.code };
  const behavior = {
    solid: await browserBehaviorDigest(
      browser,
      code.solid,
      includeCompatibility,
    ),
    solidlil: await browserBehaviorDigest(
      browser,
      code.solidlil,
      includeCompatibility,
    ),
  };
  assert.deepEqual(
    behavior.solidlil.exports,
    behavior.solid.exports,
    `${name} Solid Web exports`,
  );
  assert.deepEqual(
    behavior.solidlil.digest,
    behavior.solid.digest,
    `${name} Solid Web behavior digest`,
  );

  const sizes = {
    solid: size(code.solid),
    solidlil: size(code.solidlil),
  };
  const ratio = Object.fromEntries(
    Object.keys(sizes.solid).map((metric) => [
      metric,
      sizes.solidlil[metric] / sizes.solid[metric],
    ]),
  );
  return {
    generatedAt: new Date().toISOString(),
    baseline: `solid-js@1.9.13 ${name} browser bundle`,
    scope:
      name === "client"
        ? "client rendering; SSR and hydration excluded"
        : "complete browser export compatibility",
    buildDefines: clientOnly
      ? { "import.meta.env.SOLIDLIL_CLIENT_ONLY": true }
      : {},
    exports: behavior.solid.exports,
    exportCount: behavior.solid.exports.length,
    exactExports: true,
    behaviorEquivalent: true,
    distributionSelection: selected.selection,
    codecs: canonicalCodecProvenance("SolidLil Web surface verification"),
    compiler: buildModes.toolchain.compiler,
    sourceBuildModesSha256: createHash("sha256")
      .update(buildModesBytes)
      .digest("hex"),
    sizes,
    ratio,
    brotliSuperior: sizes.solidlil.brotli11 < sizes.solid.brotli11,
    compressedSuperior:
      sizes.solidlil.brotli11 < sizes.solid.brotli11 &&
      sizes.solidlil.gzip9 < sizes.solid.gzip9,
  };
}

const browser = await chromium.launch({ headless: true });
let report;
let clientReport;
try {
  report = await verifySurface(browser, "full", surfaces.full, true);
  clientReport = await verifySurface(browser, "client", surfaces.client, false);
} finally {
  await browser.close();
}
assert.deepEqual(report.codecs, buildModes.toolchain.codecs);
assert.deepEqual(clientReport.codecs, buildModes.toolchain.codecs);
writeFileSync(
  resolve(root, "artifacts/web-surface.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);
writeFileSync(
  resolve(root, "artifacts/web-client-surface.json"),
  `${JSON.stringify(clientReport, null, 2)}\n`,
);
writeFileSync(
  resolve(root, "artifacts/web-surface.md"),
  `# SolidLil Web verified surfaces\n\nThe release comparison is the ${clientReport.exportCount}-export client-rendering bundle. SSR and hydration are explicitly excluded from that scope. The complete ${report.exportCount}-export browser compatibility bundle remains measured separately. Both surfaces have exact export ledgers and executable behavior parity.\n\n| Surface | Metric | Solid | SolidLil | Ratio |\n| --- | --- | ---: | ---: | ---: |\n| Client rendering | Brotli-11 | ${clientReport.sizes.solid.brotli11} B | ${clientReport.sizes.solidlil.brotli11} B | ${clientReport.ratio.brotli11.toFixed(3)} |\n| Client rendering | Gzip-9 | ${clientReport.sizes.solid.gzip9} B | ${clientReport.sizes.solidlil.gzip9} B | ${clientReport.ratio.gzip9.toFixed(3)} |\n| Client rendering | Raw | ${clientReport.sizes.solid.raw} B | ${clientReport.sizes.solidlil.raw} B | ${clientReport.ratio.raw.toFixed(3)} |\n| Full compatibility | Brotli-11 | ${report.sizes.solid.brotli11} B | ${report.sizes.solidlil.brotli11} B | ${report.ratio.brotli11.toFixed(3)} |\n| Full compatibility | Gzip-9 | ${report.sizes.solid.gzip9} B | ${report.sizes.solidlil.gzip9} B | ${report.ratio.gzip9.toFixed(3)} |\n| Full compatibility | Raw | ${report.sizes.solid.raw} B | ${report.sizes.solidlil.raw} B | ${report.ratio.raw.toFixed(3)} |\n`,
);
console.log(
  `SolidLil client Web: ${clientReport.exportCount} exports verified; Brotli-11 ${clientReport.sizes.solidlil.brotli11}/${clientReport.sizes.solid.brotli11} B, gzip-9 ${clientReport.sizes.solidlil.gzip9}/${clientReport.sizes.solid.gzip9} B. Full compatibility: ${report.exportCount} exports verified.`,
);
assert.ok(clientReport.brotliSuperior, "client Web must win Brotli-11");
