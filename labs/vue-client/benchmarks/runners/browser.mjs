import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, relative, resolve } from "node:path";
import { chromium } from "playwright";
import playwrightManifest from "playwright/package.json" with { type: "json" };
import { artifactPath, emitResult } from "./common.mjs";

const root = process.cwd();
const entry = artifactPath();
const entryRelative = relative(root, entry);
if (entryRelative.startsWith("..")) throw new Error("browser artifact is outside the project");

const contentTypes = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
};
const server = createServer(async (request, response) => {
  try {
    const pathname = decodeURIComponent(new URL(request.url, "http://localhost").pathname);
    if (pathname === "/") {
      response.writeHead(200, { "content-type": contentTypes[".html"] });
      response.end("<!doctype html><html><body></body></html>");
      return;
    }
    const path = resolve(root, `.${pathname}`);
    if (relative(root, path).startsWith("..")) throw new Error("path traversal");
    const body = await readFile(path);
    response.writeHead(200, {
      "cache-control": "no-store",
      "content-type": contentTypes[extname(path)] ?? "application/octet-stream",
    });
    response.end(body);
  } catch {
    response.writeHead(404);
    response.end("not found");
  }
});
await new Promise((resolveReady, reject) => {
  server.once("error", reject);
  server.listen(0, "127.0.0.1", resolveReady);
});

let browser;
try {
  const address = server.address();
  const origin = `http://127.0.0.1:${address.port}`;
  browser = await chromium.launch({
    headless: true,
    args: ["--disable-background-timer-throttling", "--disable-renderer-backgrounding"],
  });
  const page = await browser.newPage();
  await page.goto(origin, { waitUntil: "domcontentloaded" });
  const result = await page.evaluate(async ({ moduleUrl }) => {
    globalThis.process = { env: { NODE_ENV: "production" } };
    const vue = await import(moduleUrl);
    const rowCount = 160;
    const measuredCycles = 240;
    const rows = vue.reactive(
      Array.from({ length: rowCount }, (_, id) => ({
        id,
        value: id * 7,
        active: id % 4 === 0,
      })),
    );
    const selected = vue.ref(0);
    const App = {
      setup() {
        return () => vue.h("section", {
          class: { ready: true, selected: selected.value % 2 === 0 },
          "data-selected": selected.value,
        }, [
          vue.h("button", {
            onClick: () => { selected.value = (selected.value + 17) % rowCount; },
          }, "advance"),
          vue.h("ul", null, rows.map((row) => vue.h("li", {
            key: row.id,
            class: { active: row.active },
            style: { opacity: row.active ? 1 : 0.5 },
          }, `${row.id}:${row.value}`))),
        ]);
      },
    };
    const rootElement = document.createElement("div");
    document.body.append(rootElement);
    const app = vue.createApp(App);
    app.mount(rootElement);
    const button = rootElement.querySelector("button");
    for (let cycle = 0; cycle < 20; cycle += 1) {
      const row = rows[(cycle * 13) % rowCount];
      row.value = (row.value + cycle + 1) % 100_003;
      row.active = !row.active;
      button.click();
      await vue.nextTick();
    }
    const started = performance.now();
    for (let cycle = 0; cycle < measuredCycles; cycle += 1) {
      const row = rows[(cycle * 13) % rowCount];
      row.value = (row.value * 17 + cycle) % 100_003;
      row.active = !row.active;
      button.click();
      await vue.nextTick();
    }
    const durationMs = performance.now() - started;
    const serialized = rootElement.innerHTML;
    let hash = 0x811c9dc5;
    for (let index = 0; index < serialized.length; index += 1) {
      hash ^= serialized.charCodeAt(index);
      hash = Math.imul(hash, 0x01000193);
    }
    const checksum = `${(hash >>> 0).toString(16).padStart(8, "0")}:${serialized.length}`;
    app.unmount();
    rootElement.remove();
    return {
      durationMs,
      operations: measuredCycles * (rowCount + 3),
      checksum,
    };
  }, { moduleUrl: `${origin}/${entryRelative.split("\\").join("/")}` });
  emitResult({
    ...result,
    environment: {
      engine: "Chromium",
      version: browser.version(),
      automation: `Playwright ${playwrightManifest.version}`,
      headless: true,
    },
  });
} finally {
  await browser?.close();
  await new Promise((resolveClosed) => server.close(resolveClosed));
}
