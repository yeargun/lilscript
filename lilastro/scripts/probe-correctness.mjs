import { createServer } from "node:http";
import { readFileSync } from "node:fs";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "../../benchmarks/browser/node_modules/playwright/index.mjs";

const labRoot = resolve(fileURLToPath(new URL(".", import.meta.url)));
const buildRoot = join(resolve(labRoot, ".."), "build/browser");
const FIXTURE = process.env.FIXTURE ?? "animate-play";

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
  lil: join(buildRoot, `${FIXTURE}-lil`),
  npm: join(buildRoot, `${FIXTURE}-npm`),
});
const browser = await chromium.launch({ headless: true });

for (const lane of ["npm", "lil"]) {
  const context = await browser.newContext({
    viewport: { width: 600, height: 400 },
  });
  const page = await context.newPage();
  const errors = [];
  const logs = [];
  page.on("pageerror", (e) => errors.push(e.stack || String(e)));
  page.on("console", (m) => logs.push(`${m.type()}: ${m.text()}`));
  await page.goto(`http://127.0.0.1:${port}/${lane}/index.html`, {
    waitUntil: "networkidle",
  });
  await page.waitForTimeout(800);

  const report = await page.evaluate(() => {
    const boxes = [...document.querySelectorAll(".box, #box")];
    return {
      animations: document.getAnimations().length,
      boxes: boxes.map((el) => ({
        id: el.id || el.className,
        inlineStyle: el.getAttribute("style") || "",
        transform: getComputedStyle(el).transform,
        opacity: getComputedStyle(el).opacity,
      })),
    };
  });

  console.log(`\n=== ${FIXTURE} / ${lane} ===`);
  console.log(`  live animations: ${report.animations}`);
  for (const box of report.boxes) {
    console.log(
      `  #${box.id}: transform=${box.transform} opacity=${box.opacity}\n      style="${box.inlineStyle}"`,
    );
  }
  if (errors.length) console.log(`  pageErrors: ${errors.join(" | ")}`);
  if (logs.length) console.log(`  console: ${logs.slice(0, 6).join(" | ")}`);
  await context.close();
}

await browser.close();
server.close();
