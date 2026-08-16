import { build as viteBuild } from "vite";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { readFileSync, writeFileSync } from "node:fs";

const labRoot = dirname(fileURLToPath(import.meta.url));
const buildRoot = join(labRoot, "build");

async function buildDemo(root, entry, outDir, title) {
  const indexPath = join(root, "index.html");
  const original = readFileSync(indexPath, "utf8");
  writeFileSync(
    indexPath,
    `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>${title}</title>
  </head>
  <body>
    <script type="module" src="./${entry}"></script>
  </body>
</html>
`,
  );
  try {
    await viteBuild({
      root,
      base: "./",
      configFile: false,
      logLevel: "silent",
      build: {
        outDir,
        emptyOutDir: true,
        minify: true,
        modulePreload: { polyfill: false },
      },
    });
  } finally {
    writeFileSync(indexPath, original);
  }
}

await buildDemo(
  join(labRoot, "apps/jquery/js"),
  "browser.js",
  join(buildRoot, "jquery-vite"),
  "jquery npm",
);
await buildDemo(
  join(labRoot, "apps/jquery/lil"),
  "browser.js",
  join(buildRoot, "jquery-lilscript-vite"),
  "jquery lil",
);
console.log("built jquery browser demo lanes");
