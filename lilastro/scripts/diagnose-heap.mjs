import { createServer } from "node:http";
import { readFileSync } from "node:fs";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "../../benchmarks/browser/node_modules/playwright/index.mjs";

const labRoot = resolve(fileURLToPath(new URL(".", import.meta.url)));
const lilastroRoot = resolve(labRoot, "..");
const buildRoot = join(lilastroRoot, "build/browser");

const ROUNDS = Number(process.env.ROUNDS ?? 30);
const LANES = (process.env.LANES ?? "perf-stagger-npm,perf-stagger-lil").split(
  ",",
);
const TARGET = process.env.TARGET ?? "Animation";
const PATHS = Number(process.env.PATHS ?? 3);

function startStaticServer(roots) {
  const server = createServer((request, response) => {
    const url = new URL(request.url, "http://127.0.0.1");
    const [, lane, ...rest] = url.pathname.split("/");
    const root = roots[lane];
    if (!root) {
      response.writeHead(404).end("unknown lane");
      return;
    }
    const path = resolve(root, normalize(rest.join("/") || "index.html"));
    if (!path.startsWith(root)) {
      response.writeHead(403).end();
      return;
    }
    try {
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
      response.end(readFileSync(path));
    } catch {
      response.writeHead(404).end();
    }
  });
  return new Promise((ready) => {
    server.listen(0, "127.0.0.1", () =>
      ready({ server, port: server.address().port }),
    );
  });
}

async function settledHeap(cdp) {
  await cdp.send("HeapProfiler.collectGarbage");
  await cdp.send("HeapProfiler.collectGarbage");
  const perf = await cdp.send("Performance.getMetrics");
  return perf.metrics.find((m) => m.name === "JSHeapUsedSize")?.value ?? null;
}

async function takeSnapshot(cdp) {
  const chunks = [];
  const onChunk = ({ chunk }) => chunks.push(chunk);
  cdp.on("HeapProfiler.addHeapSnapshotChunk", onChunk);
  await cdp.send("HeapProfiler.takeHeapSnapshot", {
    reportProgress: false,
    treatGlobalObjectsAsRoots: true,
  });
  cdp.off("HeapProfiler.addHeapSnapshotChunk", onChunk);
  return JSON.parse(chunks.join(""));
}

/** Indexes a v8 heap snapshot for name lookup and reverse-edge traversal. */
function indexSnapshot(snapshot) {
  const nodeFields = snapshot.snapshot.meta.node_fields;
  const edgeFields = snapshot.snapshot.meta.edge_fields;
  const nodeStride = nodeFields.length;
  const edgeStride = edgeFields.length;
  const nodeTypeIdx = nodeFields.indexOf("type");
  const nodeNameIdx = nodeFields.indexOf("name");
  const nodeSizeIdx = nodeFields.indexOf("self_size");
  const edgeCountIdx = nodeFields.indexOf("edge_count");
  const edgeTypeIdx = edgeFields.indexOf("type");
  const edgeNameIdx = edgeFields.indexOf("name_or_index");
  const edgeToIdx = edgeFields.indexOf("to_node");
  const nodeTypes = snapshot.snapshot.meta.node_types[nodeTypeIdx];
  const edgeTypes = snapshot.snapshot.meta.edge_types[edgeTypeIdx];
  const nodes = snapshot.nodes;
  const edges = snapshot.edges;
  const strings = snapshot.strings;
  const nodeCount = nodes.length / nodeStride;

  const firstEdge = new Uint32Array(nodeCount + 1);
  let cursor = 0;
  for (let i = 0; i < nodeCount; i++) {
    firstEdge[i] = cursor;
    cursor += nodes[i * nodeStride + edgeCountIdx];
  }
  firstEdge[nodeCount] = cursor;

  const retainerCount = new Uint32Array(nodeCount);
  for (let e = 0; e < edges.length; e += edgeStride) {
    retainerCount[edges[e + edgeToIdx] / nodeStride] += 1;
  }
  const retainerStart = new Uint32Array(nodeCount + 1);
  for (let i = 0; i < nodeCount; i++) {
    retainerStart[i + 1] = retainerStart[i] + retainerCount[i];
  }
  const retainerNode = new Uint32Array(edges.length / edgeStride);
  const retainerEdge = new Uint32Array(edges.length / edgeStride);
  const fill = retainerStart.slice();
  for (let i = 0; i < nodeCount; i++) {
    for (let e = firstEdge[i]; e < firstEdge[i + 1]; e++) {
      const to = edges[e * edgeStride + edgeToIdx] / nodeStride;
      const slot = fill[to]++;
      retainerNode[slot] = i;
      retainerEdge[slot] = e;
    }
  }

  const nodeName = (i) => strings[nodes[i * nodeStride + nodeNameIdx]] || "";
  const nodeType = (i) => nodeTypes[nodes[i * nodeStride + nodeTypeIdx]];
  const nodeSize = (i) => nodes[i * nodeStride + nodeSizeIdx];
  const edgeLabel = (e) => {
    const type = edgeTypes[edges[e * edgeStride + edgeTypeIdx]];
    const raw = edges[e * edgeStride + edgeNameIdx];
    const name =
      type === "element" || type === "hidden" ? `[${raw}]` : strings[raw] || "?";
    return { type, name };
  };

  return {
    nodeCount,
    nodeName,
    nodeType,
    nodeSize,
    edgeLabel,
    retainerStart,
    retainerNode,
    retainerEdge,
  };
}

function byConstructor(idx) {
  const map = new Map();
  for (let i = 0; i < idx.nodeCount; i++) {
    const key = `${idx.nodeType(i)}:${idx.nodeName(i) || "(anonymous)"}`;
    const entry = map.get(key) ?? { count: 0, size: 0 };
    entry.count += 1;
    entry.size += idx.nodeSize(i);
    map.set(key, entry);
  }
  return map;
}

/**
 * Walks reverse edges breadth-first so the printed chain is a shortest path
 * from a GC root to the leaked object.
 */
function retainerPath(idx, target) {
  const seen = new Set([target]);
  const parent = new Map();
  let frontier = [target];
  for (let depth = 0; depth < 40 && frontier.length; depth++) {
    const next = [];
    for (const node of frontier) {
      for (let r = idx.retainerStart[node]; r < idx.retainerStart[node + 1]; r++) {
        const from = idx.retainerNode[r];
        if (seen.has(from)) continue;
        seen.add(from);
        parent.set(from, { child: node, edge: idx.retainerEdge[r] });
        if (idx.nodeType(from) === "synthetic" || idx.nodeName(from) === "global") {
          const chain = [];
          let cursor = from;
          while (cursor !== undefined) {
            const step = parent.get(cursor);
            chain.push({
              name: `${idx.nodeType(cursor)}:${idx.nodeName(cursor) || "(anon)"}`,
              edge: step ? idx.edgeLabel(step.edge) : null,
            });
            cursor = step?.child;
          }
          return chain;
        }
        next.push(from);
      }
    }
    frontier = next;
  }
  return null;
}

async function runLane(browser, port, lane) {
  const context = await browser.newContext({
    viewport: { width: 500, height: 500 },
  });
  const page = await context.newPage();
  const cdp = await context.newCDPSession(page);
  await cdp.send("Performance.enable");
  await cdp.send("HeapProfiler.enable");
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("console", (m) => {
    if (m.type() === "error") errors.push(`console: ${m.text()}`);
  });
  await page.goto(`http://127.0.0.1:${port}/${lane}/index.html`, {
    waitUntil: "networkidle",
  });
  await page.waitForFunction(() => window.__perfReady === true, null, {
    timeout: 15000,
  });

  const before = indexSnapshot(await takeSnapshot(cdp));
  const heapBefore = await settledHeap(cdp);
  for (let round = 0; round < ROUNDS; round++) {
    await page.evaluate(() => {
      window.__perfSampleDone = false;
      window.__runPerfSample();
    });
    await page.waitForFunction(() => window.__perfSampleDone === true, null, {
      timeout: 10000,
    });
  }
  const heapAfter = await settledHeap(cdp);
  const snapshot = await takeSnapshot(cdp);
  await context.close();

  return {
    heapBefore,
    heapAfter,
    before: byConstructor(before),
    idx: indexSnapshot(snapshot),
    errors,
  };
}

const { server, port } = await startStaticServer({
  "perf-stagger-lil": join(buildRoot, "perf-stagger-lil"),
  "perf-stagger-npm": join(buildRoot, "perf-stagger-npm"),
});
const browser = await chromium.launch({ headless: true });

for (const lane of LANES) {
  const { heapBefore, heapAfter, before, idx, errors } = await runLane(
    browser,
    port,
    lane,
  );
  console.log(`\n=== ${lane} (${ROUNDS} rounds) ===`);
  console.log(
    `  settled heap ${(heapBefore / 1048576).toFixed(2)} MB -> ${(heapAfter / 1048576).toFixed(2)} MB`,
  );
  if (errors.length) console.log(`  errors: ${errors.slice(0, 4).join(" | ")}`);

  const after = byConstructor(idx);
  const rows = [];
  for (const [key, end] of after) {
    const start = before.get(key) ?? { count: 0, size: 0 };
    if (end.size - start.size > 40000) {
      rows.push({
        key,
        objs: end.count - start.count,
        kb: Math.round((end.size - start.size) / 1024),
      });
    }
  }
  rows.sort((a, b) => b.kb - a.kb);
  for (const row of rows.slice(0, 10)) {
    const label = row.key.length > 90 ? `${row.key.slice(0, 90)}...` : row.key;
    console.log(
      `    ${String(row.kb).padStart(6)} KB ${String(row.objs).padStart(7)} objs  ${label}`,
    );
  }

  const jsRows = [];
  let jsTotal = 0;
  for (const [key, end] of after) {
    if (key.startsWith("native:") || key.startsWith("synthetic:")) continue;
    jsTotal += end.size;
    jsRows.push({ key, objs: end.count, kb: Math.round(end.size / 1024) });
  }
  jsRows.sort((a, b) => b.kb - a.kb);
  console.log(`\n  absolute JS-side heap: ${(jsTotal / 1048576).toFixed(2)} MB`);
  for (const row of jsRows.slice(0, 8)) {
    const label = row.key.length > 70 ? `${row.key.slice(0, 70)}...` : row.key;
    console.log(
      `    ${String(row.kb).padStart(6)} KB ${String(row.objs).padStart(7)} objs  ${label}`,
    );
  }

  const targets = [];
  for (let i = 0; i < idx.nodeCount && targets.length < PATHS; i++) {
    if (idx.nodeName(i) === TARGET && idx.nodeType(i) === "native") {
      targets.push(i);
    }
  }
  for (const target of targets) {
    const chain = retainerPath(idx, target);
    console.log(`\n  retainer path for native:${TARGET} #${target}:`);
    if (!chain) {
      console.log("    (no path to root found)");
      continue;
    }
    chain.forEach((step, depth) => {
      const via = step.edge ? ` --${step.edge.type}:${step.edge.name}-->` : "";
      console.log(`    ${"  ".repeat(depth)}${step.name}${via}`);
    });
  }
}

await browser.close();
server.close();
