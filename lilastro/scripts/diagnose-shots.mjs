import { createServer } from "node:http";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "../../benchmarks/browser/node_modules/playwright/index.mjs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const lilastroRoot = resolve(labRoot, "..");
const buildRoot = join(lilastroRoot, "build/browser");
const outRoot = join(lilastroRoot, "build/diagnose-shots");
mkdirSync(outRoot, { recursive: true });

const FIXTURES = [
  {
    id: "animate-play",
    sample: async (page) => {
      const boxes = page.locator(".box");
      const count = await boxes.count();
      const rows = [];
      for (let i = 0; i < count; i++) {
        rows.push(
          await boxes.nth(i).evaluate((el) => ({
            id: el.id,
            style: el.getAttribute("style"),
            transform: getComputedStyle(el).transform,
            opacity: getComputedStyle(el).opacity,
            x: el.getBoundingClientRect().x,
          })),
        );
      }
      return rows;
    },
  },
  {
    id: "animate-css-vars",
    sample: async (page) => ({
      style: await page.locator("#box").getAttribute("style"),
      opacity: await page.locator("#box").evaluate((el) => getComputedStyle(el).opacity),
      x: (await page.locator("#box").boundingBox())?.x ?? null,
    }),
  },
  {
    id: "animate-stagger",
    sample: async (page) => {
      const boxes = page.locator(".box");
      const count = await boxes.count();
      const rows = [];
      for (let i = 0; i < count; i++) {
        rows.push(
          await boxes.nth(i).evaluate((el) => ({
            id: el.id,
            style: el.getAttribute("style"),
            transform: getComputedStyle(el).transform,
            opacity: Number(getComputedStyle(el).opacity),
            x: el.getBoundingClientRect().x,
          })),
        );
      }
      return rows;
    },
  },
  {
    id: "animate-spring",
    sample: async (page) => ({
      style: await page.locator("#box").getAttribute("style"),
      transform: await page.locator("#box").evaluate((el) => getComputedStyle(el).transform),
      x: (await page.locator("#box").boundingBox())?.x ?? null,
    }),
  },
  {
    id: "animate-scroll",
    prepare: async (page) => {
      await page.evaluate(() => window.scrollTo(0, 0));
    },
    mid: async (page) => {
      await page.evaluate(() => window.scrollTo(0, 400));
    },
    sample: async (page) => ({
      status: await page.locator("#status").textContent(),
      boxes: await page.evaluate(() =>
        [...document.querySelectorAll(".box")].map((el) => ({
          id: el.id,
          style: el.getAttribute("style"),
          transform: getComputedStyle(el).transform,
          x: el.getBoundingClientRect().x,
        })),
      ),
    }),
  },
  {
    id: "gesture-press",
    prepare: async (page) => {},
    mid: async (page) => {
      await page.locator("#box").hover();
      await page.mouse.down();
    },
    after: async (page) => {
      await page.mouse.up();
    },
    sample: async (page) => ({
      status: await page.locator("#status").textContent(),
      style: await page.locator("#box").getAttribute("style"),
      transform: await page.locator("#box").evaluate((el) => getComputedStyle(el).transform),
      scaleApprox: await page.locator("#box").evaluate((el) => {
        const t = getComputedStyle(el).transform;
        if (!t || t === "none") return 1;
        const m = t.match(/matrix\(([^)]+)\)/);
        return m ? Number(m[1].split(",")[0]) : null;
      }),
    }),
  },
  {
    id: "gesture-hover",
    prepare: async (page) => {
      await page.mouse.move(0, 0);
    },
    mid: async (page) => {
      await page.locator("#box").hover();
    },
    after: async (page) => {
      await page.mouse.move(0, 0);
    },
    sample: async (page) => ({
      status: await page.locator("#status").textContent(),
      style: await page.locator("#box").getAttribute("style"),
      transform: await page.locator("#box").evaluate((el) => getComputedStyle(el).transform),
    }),
  },
  {
    id: "in-view",
    prepare: async (page) => {
      await page.evaluate(() => window.scrollTo(0, 0));
    },
    mid: async (page) => {
      await page.locator("#box").scrollIntoViewIfNeeded();
    },
    sample: async (page) => ({
      status: await page.locator("#status").textContent(),
      opacity: await page.locator("#box").evaluate((el) => getComputedStyle(el).opacity),
      style: await page.locator("#box").getAttribute("style"),
    }),
  },
  {
    id: "resize-box",
    prepare: async (page) => {},
    mid: async (page) => {
      await page.locator("#box").evaluate((el) => {
        el.style.width = "160px";
        el.style.height = "120px";
      });
    },
    sample: async (page) => ({
      status: await page.locator("#status").textContent(),
      box: await page.locator("#box").evaluate((el) => ({
        w: el.offsetWidth,
        h: el.offsetHeight,
      })),
    }),
  },
  {
    id: "motion-value",
    sample: async (page) => ({
      status: await page.locator("#status").textContent(),
      style: await page.locator("#box").getAttribute("style"),
      transform: await page.locator("#box").evaluate((el) => getComputedStyle(el).transform),
      x: (await page.locator("#box").boundingBox())?.x ?? null,
    }),
  },
];

function startStaticServer(roots) {
  const server = createServer((request, response) => {
    const url = new URL(request.url, "http://127.0.0.1");
    const [, lane, ...rest] = url.pathname.split("/");
    const root = roots[lane];
    if (!root) {
      response.writeHead(404).end("unknown lane");
      return;
    }
    const rel = rest.join("/") || "index.html";
    const path = resolve(root, normalize(rel));
    if (!path.startsWith(root)) {
      response.writeHead(403).end();
      return;
    }
    try {
      const content = readFileSync(path);
      const type =
        extname(path) === ".js"
          ? "text/javascript"
          : extname(path) === ".css"
            ? "text/css"
            : "text/html";
      response.writeHead(200, {
        "content-type": `${type};charset=utf-8`,
        "cache-control": "no-store",
      });
      response.end(content);
    } catch {
      response.writeHead(404).end();
    }
  });
  return new Promise((resolveReady) => {
    server.listen(0, "127.0.0.1", () => {
      resolveReady({ server, port: server.address().port });
    });
  });
}

const served = {};
for (const fixture of FIXTURES) {
  for (const lane of ["npm", "lil"]) {
    served[`${fixture.id}-${lane}`] = join(buildRoot, `${fixture.id}-${lane}`);
  }
}

const { server, port } = await startStaticServer(served);
const browser = await chromium.launch({ headless: true });
const report = [];

try {
  for (const fixture of FIXTURES) {
    const entry = { id: fixture.id, lanes: {} };
    for (const lane of ["npm", "lil"]) {
      const key = `${fixture.id}-${lane}`;
      const context = await browser.newContext({
        viewport: { width: 500, height: 500 },
      });
      const page = await context.newPage();
      const errors = [];
      page.on("pageerror", (err) => errors.push(String(err)));
      const url = `http://127.0.0.1:${port}/${key}/index.html`;
      const timeline = [];
      const shot = async (label) => {
        const sample = await fixture.sample(page);
        const shotPath = join(outRoot, `${key}-${label}.png`);
        await page.screenshot({ path: shotPath, fullPage: true });
        timeline.push({ label, t: Date.now(), sample, shotPath });
      };

      await page.goto(url, { waitUntil: "networkidle" });
      if (fixture.prepare) await fixture.prepare(page);
      await shot("t0");
      await page.waitForTimeout(50);
      await shot("t50");
      if (fixture.mid) {
        await fixture.mid(page);
        await page.waitForTimeout(80);
        await shot("mid");
      } else {
        await page.waitForTimeout(100);
        await shot("t150");
        await page.waitForTimeout(150);
        await shot("t300");
      }
      if (fixture.after) {
        await fixture.after(page);
        await page.waitForTimeout(150);
        await shot("after");
      }
      await page.waitForTimeout(700);
      await shot("end");

      entry.lanes[lane] = { url, errors, timeline };
      await context.close();
      console.log(`probed ${key}`);
    }
    report.push(entry);
  }
} finally {
  await browser.close();
  server.close();
}

function summarizeDiff(fixture) {
  const npmEnd = fixture.lanes.npm.timeline.at(-1)?.sample;
  const lilEnd = fixture.lanes.lil.timeline.at(-1)?.sample;
  const npmT50 = fixture.lanes.npm.timeline.find((s) => s.label === "t50")?.sample;
  const lilT50 = fixture.lanes.lil.timeline.find((s) => s.label === "t50")?.sample;
  return {
    npmErrors: fixture.lanes.npm.errors,
    lilErrors: fixture.lanes.lil.errors,
    npmEnd,
    lilEnd,
    npmT50,
    lilT50,
  };
}

const summary = report.map((fixture) => ({
  id: fixture.id,
  ...summarizeDiff(fixture),
}));

writeFileSync(join(outRoot, "report.json"), JSON.stringify({ report, summary }, null, 2));
console.log(JSON.stringify(summary, null, 2));
console.log(`shots + report in ${outRoot}`);
