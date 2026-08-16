import { createServer } from "node:http";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve, extname, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "vite";
import { chromium } from "../../benchmarks/browser/node_modules/playwright/index.mjs";
import {
  configuredSampleCount,
  median,
  quantile,
  nonInferiorityStatistics,
} from "../../benchmarks/statistics.mjs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const lilastroRoot = resolve(labRoot, "..");
const repoRoot = resolve(lilastroRoot, "..");
const buildRoot = join(lilastroRoot, "build/playwright-browser");
const compiler = join(repoRoot, "target/release/lilscript");

const COMPILER_CONFIG = process.env.LILSCRIPT_CONFIG
  ? resolve(process.cwd(), process.env.LILSCRIPT_CONFIG)
  : join(lilastroRoot, "config/closed-world.toml");
const WARMUP_ROUNDS = Number(process.env.LILSCRIPT_MOTION_PERF_WARMUP ?? 8);
const SAMPLE_COUNT = configuredSampleCount();
const CONFIDENCE = 0.95;
const PERF_MAX_RATIO = Number(
  process.env.LILSCRIPT_MOTION_PERF_MAX_RATIO ?? 1.15,
);
const RUN_COLD = process.env.LILSCRIPT_MOTION_PERF_COLD !== "0";
const RUN_WARM = process.env.LILSCRIPT_MOTION_PERF_WARM !== "0";

mkdirSync(buildRoot, { recursive: true });

const CORRECTNESS = [
  {
    id: "animate-play",
    timeoutMs: 800,
    assert: async (page) => {
      const boxes = page.locator(".box");
      const count = await boxes.count();
      const styles = [];
      for (let i = 0; i < count; i++) {
        const style = (await boxes.nth(i).getAttribute("style")) || "";
        const transform = await boxes
          .nth(i)
          .evaluate((el) => getComputedStyle(el).transform);
        const box = await boxes.nth(i).boundingBox();
        styles.push({ style, transform, x: box?.x ?? null });
      }
      const finishedCount = await page.evaluate(
        () => window.__finishedCount ?? 0,
      );
      const styleOk = styles.every(
        (s) =>
          s.style.includes("translateX(100px)") ||
          (s.transform && s.transform !== "none" && s.x != null && s.x > 50),
      );
      const ok = styleOk && finishedCount === 3;
      return {
        ok,
        detail: { styles, finishedCount },
        message: ok
          ? "all boxes reached translateX(100px) and finished resolved"
          : "CSSOM end-state mismatch vs upstream animate-play",
      };
    },
  },
  {
    id: "animate-css-vars",
    timeoutMs: 800,
    assert: async (page) => {
      const opacity = await page
        .locator("#box")
        .evaluate((el) => getComputedStyle(el).opacity);
      const ok = Number(opacity) > 0.9;
      return {
        ok,
        detail: { opacity },
        message: ok ? "CSS var opacity reached ~1" : `opacity=${opacity}`,
      };
    },
  },
  {
    id: "animate-stagger",
    timeoutMs: 900,
    prepare: async (page) => {
      await page.waitForTimeout(120);
      const mid = await page.evaluate(() =>
        [...document.querySelectorAll(".box")].map((el) => ({
          id: el.id,
          opacity: Number(getComputedStyle(el).opacity),
          y: el.getBoundingClientRect().y,
        })),
      );
      page.__staggerMid = mid;
    },
    assert: async (page) => {
      const boxes = page.locator(".box");
      const count = await boxes.count();
      const detail = [];
      for (let i = 0; i < count; i++) {
        const opacity = await boxes
          .nth(i)
          .evaluate((el) => Number(getComputedStyle(el).opacity));
        const y = (await boxes.nth(i).boundingBox())?.y ?? null;
        detail.push({ opacity, y });
      }
      const mid = page.__staggerMid || [];
      const staggered =
        mid.length === 4 &&
        mid[0].opacity > mid[2].opacity + 0.05 &&
        mid[0].opacity > mid[3].opacity + 0.1;
      const endOk =
        detail.length === 4 && detail.every((item) => item.opacity > 0.9);
      const ok = endOk && staggered;
      return {
        ok,
        detail: { mid, end: detail },
        message: ok
          ? "stagger cascade mid + end"
          : staggered
            ? "stagger end-state mismatch"
            : "stagger delay not applied (boxes synced)",
      };
    },
  },
  {
    id: "animate-spring",
    timeoutMs: 1200,
    assert: async (page) => {
      const box = page.locator("#box");
      const transform = await box.evaluate(
        (el) => getComputedStyle(el).transform,
      );
      const x = (await box.boundingBox())?.x ?? null;
      const ok =
        (transform && transform !== "none" && x != null && x > 80) ||
        ((await box.getAttribute("style")) || "").includes("120");
      return {
        ok,
        detail: { transform, x },
        message: ok ? "spring reached ~x:120" : "spring end-state mismatch",
      };
    },
  },
  {
    id: "animate-scroll",
    timeoutMs: 600,
    prepare: async (page) => {
      await page.evaluate(() => window.scrollTo(0, 0));
      await page.waitForTimeout(100);
      const atStart = await page.evaluate(() =>
        [...document.querySelectorAll(".box")].map(
          (el) => el.getBoundingClientRect().x,
        ),
      );
      await page.evaluate(() => window.scrollTo(0, 400));
      await page.waitForTimeout(200);
      const atMid = await page.evaluate(() =>
        [...document.querySelectorAll(".box")].map(
          (el) => el.getBoundingClientRect().x,
        ),
      );
      await page.evaluate(() => window.scrollTo(0, 0));
      await page.waitForTimeout(150);
      const atEnd = await page.evaluate(() =>
        [...document.querySelectorAll(".box")].map(
          (el) => el.getBoundingClientRect().x,
        ),
      );
      page.__scrollDetail = { atStart, atMid, atEnd };
    },
    assert: async (page) => {
      const status = (await page.locator("#status").textContent()) || "";
      const detail = page.__scrollDetail || {};
      const atStart = detail.atStart || [];
      const atMid = detail.atMid || [];
      const atEnd = detail.atEnd || [];
      const scrubOk =
        atStart.length === 3 &&
        atMid.length === 3 &&
        atEnd.length === 3 &&
        atStart.every((x) => x < 5) &&
        atMid.every((x) => x > 30 && x < 90) &&
        atEnd.every((x) => x < 5);
      const ok = status.startsWith("progress:") && scrubOk;
      return {
        ok,
        detail: { status, ...detail },
        message: ok
          ? "scroll scrubs all animation lanes"
          : "scroll progress/scrub mismatch vs npm",
      };
    },
  },
  {
    id: "gesture-press",
    timeoutMs: 400,
    prepare: async (page) => {
      const box = page.locator("#box");
      await box.hover();
      await page.mouse.down();
      await page.waitForTimeout(80);
      const pressed = (await page.locator("#status").textContent()) || "";
      const pressedScale = await box.evaluate((el) => {
        const t = getComputedStyle(el).transform;
        if (!t || t === "none") return 1;
        const m = t.match(/matrix\(([^)]+)\)/);
        return m ? Number(m[1].split(",")[0]) : null;
      });
      await page.mouse.up();
      await page.waitForTimeout(120);
      page.__pressDetail = {
        pressed,
        pressedScale,
        released: (await page.locator("#status").textContent()) || "",
      };
    },
    assert: async (page) => {
      const detail = page.__pressDetail || {};
      const scaleOk =
        typeof detail.pressedScale === "number" &&
        detail.pressedScale > 0.75 &&
        detail.pressedScale < 1.05;
      const ok =
        detail.pressed === "pressed" &&
        detail.released === "released" &&
        scaleOk;
      return {
        ok,
        detail,
        message: ok
          ? "press/release callbacks fired"
          : "press gesture mismatch (status or mid-scale)",
      };
    },
  },
  {
    id: "gesture-hover",
    timeoutMs: 400,
    prepare: async (page) => {
      const box = page.locator("#box");
      await box.hover();
      await page.waitForTimeout(100);
      const hovered = (await page.locator("#status").textContent()) || "";
      const hoveredScale = await box.evaluate((el) => {
        const t = getComputedStyle(el).transform;
        if (!t || t === "none") return 1;
        const m = t.match(/matrix\(([^)]+)\)/);
        return m ? Number(m[1].split(",")[0]) : null;
      });
      await page.mouse.move(0, 0);
      await page.waitForTimeout(100);
      page.__hoverDetail = {
        hovered,
        hoveredScale,
        left: (await page.locator("#status").textContent()) || "",
      };
    },
    assert: async (page) => {
      const detail = page.__hoverDetail || {};
      const scaleOk =
        typeof detail.hoveredScale === "number" &&
        detail.hoveredScale > 1.02 &&
        detail.hoveredScale < 1.15;
      const ok =
        detail.hovered === "hovered" && detail.left === "left" && scaleOk;
      return {
        ok,
        detail,
        message: ok
          ? "hover enter/leave fired"
          : "hover gesture mismatch (status or mid-scale)",
      };
    },
  },
  {
    id: "in-view",
    timeoutMs: 500,
    prepare: async (page) => {
      await page.locator("#box").scrollIntoViewIfNeeded();
      await page.waitForTimeout(300);
    },
    assert: async (page) => {
      const status = (await page.locator("#status").textContent()) || "";
      const opacity = await page
        .locator("#box")
        .evaluate((el) => Number(getComputedStyle(el).opacity));
      const ok = status === "in" && opacity > 0.8;
      return {
        ok,
        detail: { status, opacity },
        message: ok ? "inView entered" : "inView mismatch",
      };
    },
  },
  {
    id: "resize-box",
    timeoutMs: 400,
    prepare: async (page) => {
      await page.locator("#box").evaluate((el) => {
        el.style.width = "160px";
        el.style.height = "120px";
      });
      await page.waitForTimeout(200);
    },
    assert: async (page) => {
      const status = (await page.locator("#status").textContent()) || "";
      const ok = status === "160x120" || status.includes("160");
      return {
        ok,
        detail: { status },
        message: ok ? "resize observed size" : `status=${status}`,
      };
    },
  },
  {
    id: "motion-value",
    timeoutMs: 800,
    assert: async (page) => {
      const status = Number(
        (await page.locator("#status").textContent()) || "0",
      );
      const x = (await page.locator("#box").boundingBox())?.x ?? null;
      const ok = status >= 180 && x != null && x > 150;
      return {
        ok,
        detail: { status, x },
        message: ok ? "motionValue + element animated" : "motionValue mismatch",
      };
    },
  },
];

function mean(values) {
  if (values.length === 0) return 0;
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function summarize(values) {
  return {
    samples: values.length,
    mean: mean(values),
    median: median(values),
    p95: quantile(values, 0.95),
    min: Math.min(...values),
    max: Math.max(...values),
  };
}

function seedFor(label) {
  let seed = 0x811c9dc5;
  for (const character of label) {
    seed ^= character.codePointAt(0);
    seed = Math.imul(seed, 0x01000193);
  }
  return seed >>> 0;
}

function randomGenerator(seed) {
  let state = seed || 0x6d2b79f5;
  return () => {
    state = Math.imul(state ^ (state >>> 15), state | 1);
    state ^= state + Math.imul(state ^ (state >>> 7), state | 61);
    return ((state ^ (state >>> 14)) >>> 0) / 4294967296;
  };
}

function run(program, args, cwd = lilastroRoot) {
  const result = spawnSync(program, args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(
      `${program} ${args.join(" ")}\n${result.stdout}\n${result.stderr}`,
    );
  }
  return result.stdout.trim();
}

async function viteBuild(root, name) {
  const outDir = join(buildRoot, name);
  await build({
    root,
    base: "./",
    logLevel: "error",
    build: {
      outDir,
      emptyOutDir: true,
      minify: false,
      write: true,
      rollupOptions: { input: join(root, "index.html") },
    },
  });
  return outDir;
}

function compileLil(fixtureId) {
  const lilDir = join(lilastroRoot, "browser", fixtureId, "lil");
  const lilMain = join(lilDir, "main.lil");
  const outJs = join(lilDir, "main.js");
  const args = [lilMain, "--target", "js", "-o", outJs];
  if (COMPILER_CONFIG) {
    args.push("--config", COMPILER_CONFIG);
  }
  run(compiler, args);
  return lilDir;
}

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

async function readCdpHeap(cdp) {
  if (!cdp) return null;
  await cdp.send("HeapProfiler.collectGarbage");
  const perf = await cdp.send("Performance.getMetrics");
  const used = perf.metrics.find((metric) => metric.name === "JSHeapUsedSize");
  return used?.value ?? null;
}

async function collectSample(page, cdp) {
  await page.evaluate(() => {
    window.__runPerfSample();
  });
  await page.waitForFunction(() => window.__perfSampleDone === true, null, {
    timeout: 8000,
  });
  const sample = await page.evaluate(() => window.__perfSample);
  const heapUsed = await readCdpHeap(cdp);
  return {
    animateMs: Number(sample.animateMs ?? sample.scheduleMs),
    scheduleMs: Number(sample.scheduleMs ?? sample.animateMs),
    frameMean: Number(sample.frameMean),
    frameP95: Number(sample.frameP95),
    heapDelta: Number(sample.heapDelta),
    heapUsed: heapUsed == null ? Number(sample.heapEnd) : heapUsed,
  };
}

async function openLane(browser, port, laneKey) {
  const context = await browser.newContext({
    viewport: { width: 500, height: 500 },
  });
  const page = await context.newPage();
  const cdp = await context.newCDPSession(page);
  await cdp.send("Performance.enable");
  const errors = [];
  page.on("pageerror", (err) => errors.push(String(err)));
  const url = `http://127.0.0.1:${port}/${laneKey}/index.html`;
  await page.goto(url, { waitUntil: "networkidle" });
  try {
    await page.waitForFunction(() => window.__perfReady === true, null, {
      timeout: 10000,
    });
  } catch (err) {
    await context.close();
    throw new Error(
      `${laneKey} failed to become ready: ${err}\npageErrors=${errors.join(" | ") || "none"}`,
    );
  }
  return { context, page, cdp, url, errors };
}

/**
 * Paired rounds: each index is one machine-load round with randomized lane
 * order. Warmup rounds are discarded. Cold reloads the document each sample;
 * warm reuses the page and re-runs __runPerfSample.
 */
async function runPairedPerf({
  browser,
  port,
  label,
  mode,
  sampleCount,
  warmupRounds,
}) {
  const random = randomGenerator(seedFor(`${label}:${mode}`));
  const metrics = {
    scheduleMs: { lil: [], npm: [] },
    frameP95: { lil: [], npm: [] },
    frameMean: { lil: [], npm: [] },
    heapUsed: { lil: [], npm: [] },
  };
  const orderCounts = { lilFirst: 0, npmFirst: 0 };
  const pageErrors = { lil: [], npm: [] };

  let warm = null;
  if (mode === "warm") {
    warm = {
      lil: await openLane(browser, port, "perf-stagger-lil"),
      npm: await openLane(browser, port, "perf-stagger-npm"),
    };
  }

  try {
    const totalRounds = warmupRounds + sampleCount;
    for (let round = 0; round < totalRounds; round += 1) {
      const lilFirst = random() < 0.5;
      const order = lilFirst ? ["lil", "npm"] : ["npm", "lil"];
      if (lilFirst) orderCounts.lilFirst += 1;
      else orderCounts.npmFirst += 1;

      const roundSample = { lil: null, npm: null };
      for (const lane of order) {
        let handle;
        if (mode === "cold") {
          handle = await openLane(browser, port, `perf-stagger-${lane}`);
        } else {
          handle = warm[lane];
        }
        try {
          roundSample[lane] = await collectSample(handle.page, handle.cdp);
          if (handle.errors.length) {
            pageErrors[lane].push(...handle.errors.splice(0));
          }
        } finally {
          if (mode === "cold") {
            await handle.context.close();
          }
        }
      }

      if (round < warmupRounds) continue;
      for (const metricName of Object.keys(metrics)) {
        metrics[metricName].lil.push(roundSample.lil[metricName]);
        metrics[metricName].npm.push(roundSample.npm[metricName]);
      }
      if ((round + 1) % 25 === 0 || round + 1 === totalRounds) {
        console.log(
          `  ${mode} round ${round + 1}/${totalRounds} (kept ${Math.max(0, round + 1 - warmupRounds)}/${sampleCount})`,
        );
      }
    }
  } finally {
    if (warm) {
      await warm.lil.context.close();
      await warm.npm.context.close();
    }
  }

  const comparisons = {};
  for (const [metricName, lanes] of Object.entries(metrics)) {
    const stats = nonInferiorityStatistics(lanes.lil, lanes.npm, {
      label: `${label}/${mode}/${metricName}`,
      confidence: CONFIDENCE,
    });
    comparisons[metricName] = {
      lil: summarize(lanes.lil),
      npm: summarize(lanes.npm),
      ratio: stats.ratio,
      upperConfidenceRatio: stats.upperConfidenceRatio,
      confidence: stats.confidence,
      bootstrapSamples: stats.bootstrapSamples,
      withinBudget:
        stats.upperConfidenceRatio.median <= PERF_MAX_RATIO &&
        stats.upperConfidenceRatio.p95 <= PERF_MAX_RATIO,
    };
  }

  return {
    mode,
    warmupRounds,
    sampleCount,
    confidence: CONFIDENCE,
    maxRatio: PERF_MAX_RATIO,
    orderCounts,
    pageErrors,
    comparisons,
    ok: Object.values(comparisons).every((item) => item.withinBudget),
  };
}

const results = {
  generatedAt: new Date().toISOString(),
  methodology: {
    buildMode: "closed-world",
    compilerConfig: COMPILER_CONFIG,
    pairing: "same-round machine load; randomized lil/npm order per round",
    warmupRounds: WARMUP_ROUNDS,
    sampleCount: SAMPLE_COUNT,
    confidence: CONFIDENCE,
    bootstrap: "paired index bootstrap from benchmarks/statistics.mjs",
    cold: "fresh document navigation per sample",
    warm: "one page per lane; repeated __runPerfSample",
    metrics: ["scheduleMs", "frameMean", "frameP95", "heapUsedAfterGc"],
    maxRatio: PERF_MAX_RATIO,
    surface:
      "192-element animateMini WAAPI stagger across transform and opacity (paired lil vs npm); scheduleMs is sync call cost; frames and forced-GC retained heap are measured over a fixed animation window",
  },
  upstream: {
    repo: "https://github.com/motiondivision/motion",
    tag: "v13.0.0",
    playwrightSource: "dev/html/public/playwright/animate + tests/animate",
  },
  correctness: [],
  performance: [],
};

console.log("building browser fixtures...");
const served = {};
const fixtureIds = [...CORRECTNESS.map((item) => item.id), "perf-stagger"];
for (const fixtureId of fixtureIds) {
  console.log(`  npm ${fixtureId}`);
  const npmOut = await viteBuild(
    join(lilastroRoot, "browser", fixtureId, "npm"),
    `${fixtureId}-npm`,
  );
  console.log(`  lil ${fixtureId}`);
  const lilApp = compileLil(fixtureId);
  const lilOut = await viteBuild(lilApp, `${fixtureId}-lil`);
  served[`${fixtureId}-npm`] = npmOut;
  served[`${fixtureId}-lil`] = lilOut;
}

const { server, port } = await startStaticServer(served);
const browser = await chromium.launch({ headless: true });

try {
  for (const fixture of CORRECTNESS) {
    const entry = { id: fixture.id, kind: "correctness", lanes: {} };
    for (const lane of ["npm", "lil"]) {
      const key = `${fixture.id}-${lane}`;
      console.log(`playwright correctness ${key}...`);
      const context = await browser.newContext({
        viewport: { width: 500, height: 500 },
      });
      const page = await context.newPage();
      const errors = [];
      page.on("pageerror", (err) => errors.push(String(err)));
      const url = `http://127.0.0.1:${port}/${key}/index.html`;
      let assertion = { ok: false, message: "not run", detail: null };
      try {
        // These fixtures start their animation during module evaluation. Waiting
        // for network-idle adds a fixed 500ms and can miss the stagger entirely.
        await page.goto(url, { waitUntil: "load" });
        if (typeof fixture.prepare === "function") {
          await fixture.prepare(page);
        }
        await page.waitForTimeout(fixture.timeoutMs);
        assertion = await fixture.assert(page);
      } catch (err) {
        assertion = { ok: false, message: String(err), detail: null };
      }
      entry.lanes[lane] = {
        url,
        ok: assertion.ok && errors.length === 0,
        message: assertion.message,
        detail: assertion.detail,
        pageErrors: errors,
      };
      await context.close();
    }
    results.correctness.push(entry);
  }

  if (RUN_WARM) {
    console.log(
      `playwright perf warm (${SAMPLE_COUNT} paired samples, ${WARMUP_ROUNDS} warmup, random order)...`,
    );
    results.performance.push(
      await runPairedPerf({
        browser,
        port,
        label: "perf-stagger",
        mode: "warm",
        sampleCount: SAMPLE_COUNT,
        warmupRounds: WARMUP_ROUNDS,
      }),
    );
  }

  if (RUN_COLD) {
    console.log(
      `playwright perf cold (${SAMPLE_COUNT} paired samples, ${WARMUP_ROUNDS} warmup, random order)...`,
    );
    results.performance.push(
      await runPairedPerf({
        browser,
        port,
        label: "perf-stagger",
        mode: "cold",
        sampleCount: SAMPLE_COUNT,
        warmupRounds: WARMUP_ROUNDS,
      }),
    );
  }
} catch (err) {
  results.error = String(err);
  throw err;
} finally {
  await browser.close();
  server.close();
  const correctnessOnly = !RUN_WARM && !RUN_COLD;
  const outPath = join(
    lilastroRoot,
    correctnessOnly
      ? "build/playwright-correctness-results.json"
      : "build/playwright-results.json",
  );
  writeFileSync(outPath, JSON.stringify(results, null, 2));
  console.log(`wrote ${outPath}`);
}

const failedCorrectness = results.correctness.flatMap((fixture) =>
  Object.entries(fixture.lanes)
    .filter(([, lane]) => !lane.ok)
    .map(([lane]) => `${fixture.id}/${lane}`),
);
const failedPerf = results.performance
  .filter((entry) => !entry.ok)
  .map((entry) => `perf-stagger/${entry.mode}`);

if (failedCorrectness.length || failedPerf.length || results.error) {
  console.error(
    `FAILED: ${[...failedCorrectness, ...failedPerf, results.error].filter(Boolean).join(", ")}`,
  );
  process.exitCode = 1;
} else {
  console.log(
    RUN_WARM || RUN_COLD
      ? "all playwright correctness + statistical perf gates passed"
      : "all playwright correctness gates passed (performance skipped)",
  );
}
