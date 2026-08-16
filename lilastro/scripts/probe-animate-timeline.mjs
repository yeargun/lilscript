import { createServer } from "node:http";
import { readFileSync } from "node:fs";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "../../benchmarks/browser/node_modules/playwright/index.mjs";

const labRoot = resolve(fileURLToPath(new URL(".", import.meta.url)));
const buildRoot = join(resolve(labRoot, ".."), "build/browser");

function startStaticServer(roots) {
  const server = createServer((request, response) => {
    const url = new URL(request.url, "http://127.0.0.1");
    const [, lane, ...rest] = url.pathname.split("/");
    const root = roots[lane];
    if (!root) return void response.writeHead(404).end();
    const path = resolve(root, normalize(rest.join("/") || "index.html"));
    if (!path.startsWith(root)) return void response.writeHead(403).end();
    try {
      const type = extname(path) === ".js" ? "text/javascript" : "text/html";
      response.writeHead(200, {
        "content-type": `${type};charset=utf-8`,
        "cache-control": "no-store",
      });
      response.end(readFileSync(path));
    } catch {
      response.writeHead(404).end();
    }
  });
  return new Promise((ready) =>
    server.listen(0, "127.0.0.1", () =>
      ready({ server, port: server.address().port }),
    ),
  );
}

const { server, port } = await startStaticServer({
  lil: join(buildRoot, "animate-play-lil"),
});
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage();
const errors = [];
page.on("pageerror", (e) => errors.push(e.stack || String(e)));
page.on("console", (m) => {
  if (m.type() === "error" || m.type() === "warning") {
    console.log(`console.${m.type()}: ${m.text()}`);
  }
});

await page.goto(`http://127.0.0.1:${port}/lil/index.html`, {
  waitUntil: "domcontentloaded",
});

for (const ms of [0, 50, 100, 150, 250, 400, 700]) {
  await page.waitForTimeout(ms === 0 ? 0 : ms - (ms > 50 ? 50 : 0));
  if (ms === 0) await page.waitForTimeout(20);
  const snap = await page.evaluate(() => {
    const js = document.querySelector("#js");
    const waapi = document.querySelector("#waapi");
    return {
      live: document.getAnimations().map((a) => ({
        playState: a.playState,
        effect: a.effect?.getTiming?.(),
        target: a.effect?.target?.id,
      })),
      js: {
        style: js?.getAttribute("style") || "",
        transform: getComputedStyle(js).transform,
      },
      waapi: {
        style: waapi?.getAttribute("style") || "",
        transform: getComputedStyle(waapi).transform,
      },
    };
  });
  console.log(`t≈${ms}ms live=${snap.live.length}`, JSON.stringify(snap.live), "js=", snap.js, "waapi=", snap.waapi);
}

if (errors.length) console.log("errors:", errors.join("\n"));
await browser.close();
server.close();
