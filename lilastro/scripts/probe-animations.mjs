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
  "perf-stagger-lil": join(buildRoot, "perf-stagger-lil"),
  "perf-stagger-npm": join(buildRoot, "perf-stagger-npm"),
});
const browser = await chromium.launch({ headless: true });

for (const lane of ["perf-stagger-npm", "perf-stagger-lil"]) {
  const context = await browser.newContext({
    viewport: { width: 500, height: 500 },
  });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto(`http://127.0.0.1:${port}/${lane}/index.html`, {
    waitUntil: "networkidle",
  });
  await page.waitForFunction(() => window.__perfReady === true);

  const report = await page.evaluate(async () => {
    const snap = (label) => {
      const anims = document.getAnimations();
      const states = {};
      for (const a of anims) {
        const key = `${a.playState}/${a.effect?.getComputedTiming?.().duration}/fill=${a.effect?.getTiming?.().fill}/iter=${a.effect?.getTiming?.().iterations}`;
        states[key] = (states[key] ?? 0) + 1;
      }
      return { label, count: anims.length, states };
    };

    const trail = [];
    for (let round = 0; round < 3; round++) {
      window.__perfSampleDone = false;
      window.__runPerfSample();
      await new Promise((r) => {
        const check = () =>
          window.__perfSampleDone ? r() : setTimeout(check, 20);
        check();
      });
      trail.push(snap(`after round ${round}`));
    }
    await new Promise((r) => setTimeout(r, 600));
    trail.push(snap("after 600ms settle"));
    return trail;
  });

  console.log(`\n=== ${lane} ===`);
  for (const point of report) {
    console.log(`  ${point.label}: document.getAnimations() = ${point.count}`);
    for (const [state, count] of Object.entries(point.states)) {
      console.log(`      ${count} x ${state}`);
    }
  }
  if (errors.length) console.log("  errors:", errors.slice(0, 3).join(" | "));
  await context.close();
}

await browser.close();
server.close();
