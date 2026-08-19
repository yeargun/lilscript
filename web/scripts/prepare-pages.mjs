import { readdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

const dist = join(import.meta.dirname, "../dist");
const base = (process.env.LILSCRIPT_PAGES_BASE ?? "/lilscript/").replace(/\/?$/, "/");
const prefix = base.replace(/\/$/, "");

function rewrite(html) {
  return html
    .replaceAll('href="/"', `href="${base}"`)
    .replace(/(href|src)="\/(?!\/)([^"]*)"/g, (match, attr, path) => {
      if (path.startsWith("lilscript/") || path.startsWith("api/")) return match;
      return `${attr}="${prefix}/${path}"`;
    });
}

async function walk(dir) {
  const entries = await readdir(dir, { withFileTypes: true });
  for (const entry of entries) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      await walk(path);
      continue;
    }
    if (!entry.name.endsWith(".html")) continue;
    const before = await readFile(path, "utf8");
    const after = rewrite(before);
    if (after !== before) await writeFile(path, after);
  }
}

await writeFile(join(dist, ".nojekyll"), "");
await walk(dist);
console.log(`prefixed root hrefs with ${base}`);
