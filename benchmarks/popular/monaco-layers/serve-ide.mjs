import { createServer } from "node:http";
import { existsSync, readFileSync, statSync } from "node:fs";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { brotliCompressSync, constants as zlibConstants, gzipSync } from "node:zlib";

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

const packed = new Map();

function safePath(urlPath) {
  const decoded = decodeURIComponent((urlPath.split("?")[0] || "/"));
  const rel = decoded === "/" ? "/apps/monaco/" : decoded;
  const full = normalize(join(labRoot, rel));
  if (!full.startsWith(labRoot)) {
    return null;
  }
  return full;
}

function encodingFor(req, ext) {
  const accept = req.headers["accept-encoding"] || "";
  if (!{ ".js": 1, ".css": 1, ".html": 1, ".json": 1, ".mjs": 1, ".svg": 1 }[ext]) {
    return "";
  }
  if (/\bbr\b/.test(accept)) return "br";
  if (/\bgzip\b/.test(accept)) return "gzip";
  return "";
}

function pack(body, kind) {
  if (kind === "br") {
    return brotliCompressSync(body, {
      params: {
        [zlibConstants.BROTLI_PARAM_QUALITY]: 11,
        [zlibConstants.BROTLI_PARAM_SIZE_HINT]: body.length,
      },
    });
  }
  if (kind === "gzip") {
    return gzipSync(body, { level: 9 });
  }
  return body;
}

const server = createServer((req, res) => {
  const full = safePath(req.url || "/");
  if (!full) {
    res.writeHead(403);
    res.end("forbidden");
    return;
  }
  let path = full;
  if (existsSync(path) && statSync(path).isDirectory()) {
    path = join(path, "index.html");
  }
  if (!existsSync(path) || !statSync(path).isFile()) {
    res.writeHead(404);
    res.end("not found: " + (req.url || "/"));
    return;
  }
  const ext = extname(path).toLowerCase();
  const type = mime[ext] || "application/octet-stream";
  const body = readFileSync(path);
  const kind = encodingFor(req, ext);
  const key = path + ":" + kind + ":" + body.length;
  let payload = body;
  if (kind) {
    let cached = packed.get(key);
    if (!cached) {
      cached = pack(body, kind);
      packed.set(key, cached);
    }
    payload = cached;
  }
  const headers = {
    "content-type": type,
    "cache-control": "no-store",
    vary: "accept-encoding",
  };
  if (kind) headers["content-encoding"] = kind;
  res.writeHead(200, headers);
  res.end(payload);
});

server.listen(port, host, () => {
  const base = `http://${host}:${port}`;
  console.log(`serving ${labRoot}`);
  console.log(`landing     ${base}/apps/monaco/`);
  console.log(`LilScript   ${base}/apps/monaco/lil/`);
  console.log(`JS monaco   ${base}/apps/monaco/js/`);
});
