import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createServer } from "node:net";
import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const lilpack = process.argv[2] ?? path.join(root, "target/release/lilpack");
const compiler = process.argv[3] ?? path.join(root, "target/release/lilscript");
const verification = path.join(root, "target/verification/lilpack");
const app = path.join(verification, "app");
const dist = path.join(verification, "dist");
const sourceMapDist = path.join(verification, "dist-source-map");

await rm(verification, { recursive: true, force: true });
await mkdir(verification, { recursive: true });
await cp(path.join(root, "tests/bundles/foreign"), app, { recursive: true });

assert.throws(
  () =>
    execFileSync(
      lilpack,
      ["build", path.join(app, "main.lil"), "--root", app, "--out-dir", app],
      {
        cwd: root,
        env: { ...process.env, LILSCRIPT_COMPILER: compiler },
        stdio: "pipe",
      },
    ),
  (error) => /refusing to empty broad output directory/u.test(error.stderr),
);

execFileSync(
  lilpack,
  ["build", path.join(app, "main.lil"), "--root", app, "--out-dir", dist],
  {
    cwd: root,
    env: { ...process.env, LILSCRIPT_COMPILER: compiler },
    stdio: "inherit",
  },
);

const manifest = JSON.parse(
  await readFile(path.join(dist, "lilpack.manifest.json"), "utf8"),
);
assert.equal(manifest["index.html"].isEntry, true);
const bundle = await readFile(
  path.join(dist, manifest["index.html"].file),
  "utf8",
);
assert.match(bundle, /return\s+[A-Za-z_$][\w$]*\+[A-Za-z_$][\w$]*/u);
assert.match(bundle, /22/u);
assert.doesNotMatch(bundle, /:\s*number/u);
assert.doesNotMatch(bundle, /\.ts["']/u);
assert.doesNotMatch(bundle, /modulepreload|MutationObserver/u);

execFileSync(
  lilpack,
  [
    "build",
    path.join(app, "main.lil"),
    "--root",
    app,
    "--out-dir",
    sourceMapDist,
    "--config",
    path.join(root, "tests/bundles/foreign/lilscript-sourcemap.toml"),
    "--sourcemap",
  ],
  {
    cwd: root,
    env: { ...process.env, LILSCRIPT_COMPILER: compiler },
    stdio: "inherit",
  },
);
const sourceMapManifest = JSON.parse(
  await readFile(path.join(sourceMapDist, "lilpack.manifest.json"), "utf8"),
);
const sourceMapEntry = sourceMapManifest["index.html"].file;
const mappedBundle = await readFile(path.join(sourceMapDist, sourceMapEntry), "utf8");
assert.match(mappedBundle, /\/\/# sourceMappingURL=.*\.js\.map/u);
const composedMap = JSON.parse(
  await readFile(path.join(sourceMapDist, `${sourceMapEntry}.map`), "utf8"),
);
assert.equal(composedMap.version, 3);
assert.ok(composedMap.sources.some((source) => source.endsWith("main.lil")));
assert.ok(composedMap.sources.some((source) => source.endsWith("value.lil")));
assert.ok(composedMap.sourcesContent.some((source) => source.includes("hotAccept")));
assert.ok(composedMap.sourcesContent.some((source) => source.includes("return 20")));

const port = await freePort();
const server = spawn(
  lilpack,
  ["dev", path.join(app, "main.lil"), "--root", app, "--port", String(port)],
  {
    cwd: root,
    env: { ...process.env, LILSCRIPT_COMPILER: compiler },
    stdio: ["ignore", "pipe", "pipe"],
  },
);
let output = "";
server.stdout.on("data", (chunk) => (output += chunk));
server.stderr.on("data", (chunk) => (output += chunk));

try {
  const base = `http://127.0.0.1:${port}`;
  await waitFor(async () => (await fetch(base)).ok, 15_000);
  const html = await fetchText(`${base}/`);
  assert.match(html, /\/@vite\/client/u);
  assert.match(html, /main\.lil\?import/u);

  const lilscript = await fetchText(`${base}/main.lil?import`);
  assert.match(lilscript, /from\s*["']\/host\.ts["']/u);
  assert.match(lilscript, /import\.meta\.hot\.accept/u);

  const typescript = await fetchText(`${base}/host.ts`);
  assert.match(typescript, /from\s*["']\/math\.ts["']/u);
  assert.doesNotMatch(typescript, /:\s*number/u);

  await writeFile(
    path.join(app, "value.lil"),
    "export int left() {\n  return 21;\n}\n",
  );
  await waitFor(async () => {
    const code = await fetchText(`${base}/main.lil?import&t=${Date.now()}`);
    return /21/u.test(code);
  }, 15_000);

  await writeFile(
    path.join(app, "math.ts"),
    "export function sum(left: number, right: number): number {\n  return left - right;\n}\n",
  );
  await waitFor(async () => {
    const code = await fetchText(`${base}/math.ts?t=${Date.now()}`);
    return /left\s*-\s*right/u.test(code) && !/:\s*number/u.test(code);
  }, 15_000);
} catch (error) {
  throw new Error(`${error.message}\nLilpack output:\n${output}`, { cause: error });
} finally {
  server.kill("SIGTERM");
  await Promise.race([
    new Promise((resolve) => server.once("exit", resolve)),
    new Promise((resolve) => setTimeout(resolve, 2_000)),
  ]);
}

console.log("Lilpack Vite build, composed source maps, TypeScript delivery, and hot reload passed.");

async function fetchText(url) {
  const response = await fetch(url);
  assert.equal(response.status, 200, `${url} returned ${response.status}`);
  return response.text();
}

async function waitFor(check, timeout) {
  const deadline = Date.now() + timeout;
  let lastError;
  while (Date.now() < deadline) {
    try {
      if (await check()) return;
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 75));
  }
  throw lastError ?? new Error(`condition was not met within ${timeout}ms`);
}

async function freePort() {
  const server = createServer();
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  await new Promise((resolve) => server.close(resolve));
  return address.port;
}
