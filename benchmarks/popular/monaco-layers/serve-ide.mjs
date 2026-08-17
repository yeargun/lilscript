import { createServer } from "node:http";
import { existsSync, readFileSync, statSync } from "node:fs";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const labRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const port = Number(process.env.PORT || 8787);
const host = process.env.HOST || "127.0.0.1";

const mime = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".map": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".ttf": "font/ttf",
  ".woff": "font/woff",
  ".woff2": "font/woff2",
  ".wasm": "application/wasm",
};

function safePath(urlPath) {
  const decoded = decodeURIComponent((urlPath.split("?")[0] || "/"));
  const rel = decoded === "/" ? "/apps/monaco/" : decoded;
  const full = normalize(join(labRoot, rel));
  if (!full.startsWith(labRoot)) {
    return null;
  }
  return full;
}

function send(res, status, body, type) {
  res.writeHead(status, {
    "content-type": type || "text/plain; charset=utf-8",
    "cache-control": "no-store",
  });
  res.end(body);
}

const server = createServer((req, res) => {
  const full = safePath(req.url || "/");
  if (!full) {
    send(res, 403, "forbidden");
    return;
  }
  let path = full;
  if (existsSync(path) && statSync(path).isDirectory()) {
    path = join(path, "index.html");
  }
  if (!existsSync(path) || !statSync(path).isFile()) {
    send(res, 404, "not found: " + (req.url || "/"));
    return;
  }
  const type = mime[extname(path).toLowerCase()] || "application/octet-stream";
  send(res, 200, readFileSync(path), type);
});

server.listen(port, host, () => {
  const base = `http://${host}:${port}`;
  console.log(`serving ${labRoot}`);
  console.log(`landing     ${base}/apps/monaco/`);
  console.log(`LilScript   ${base}/apps/monaco/lil/`);
  console.log(`JS monaco   ${base}/apps/monaco/js/`);
});
