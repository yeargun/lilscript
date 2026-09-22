// A measured upstream production ESM lane. Dependency installation is excluded.
import { readFileSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";
import { performance } from "node:perf_hooks";
const root = resolve(process.argv[2]);
const config = JSON.parse(readFileSync(join(root, "page-audit-config.json"), "utf8"));
const deps = join(root, "benchmarks/page-audit/node_modules");
const { build, transform } = await import(pathToFileURL(join(deps, "esbuild/lib/main.js")));
const { minify } = await import(pathToFileURL(join(deps, "terser/main.js")));
const out = join(root, ".page-audit/upstream");
mkdirSync(out, { recursive: true });
let external = config.external ?? [];
let candidateExports = null;
if (config.primaryArtifact && existsSync(join(root, config.primaryArtifact))) {
  const scan = await build({ entryPoints:[join(root,config.primaryArtifact)], bundle:true, packages:"external", format:"esm", platform:"browser", write:false, metafile:true, logLevel:"error" });
  const entry = Object.values(scan.metafile.outputs).find(output => output.entryPoint);
  candidateExports = entry?.exports ?? null;
  external = [...new Set([...external, ...(entry?.imports ?? []).filter(item => item.external && !item.path.startsWith('.')).map(item => item.path.replace(/^@itslil\//,''))])];
}
const samples = [];
let bundled, compressed;
for (let run = 0; run < 3; run++) {
  const start = performance.now();
  if (config.name === "markedlil") {
    const { bundleOfficialParseOnly } = await import(pathToFileURL(join(root, "scripts/official-parse-bundle.mjs")));
    bundled = await bundleOfficialParseOnly(root);
  } else if (config.name === "posthoglil") {
    const { bundleOfficialKernel } = await import(pathToFileURL(join(root, "scripts/official-bundle.mjs")));
    bundled = await bundleOfficialKernel(root);
  } else {
    const result = await build({
      absWorkingDir: join(root, "benchmarks/page-audit"),
      stdin: { contents: config.upstreamEntry, resolveDir: join(root, "benchmarks/page-audit"), sourcefile: "entry.mjs" },
      bundle: true, format: "esm", platform: "browser", target: "esnext", write: false,
      legalComments: "none", external,
      outdir: out, loader: { ".ttf": "file" },
      define: { "process.env.NODE_ENV": '"production"' },
      logLevel: "error",
    });
    bundled = result.outputFiles.find(file => file.path.endsWith('.js')).text;
    for (const file of result.outputFiles.filter(file => !file.path.endsWith('.js'))) writeFileSync(file.path, file.contents);
  }
  const bundledAt = performance.now();
  compressed = (await minify(bundled, { module: true, compress: { passes: 3 }, mangle: true, format: { comments: false } })).code;
  if (!compressed) throw new Error("Upstream minifier emitted no artifact");
  writeFileSync(join(out, "official.esm.js"), compressed + "\n");
  samples.push({ bundleSeconds: (bundledAt - start) / 1000, wallSeconds: (performance.now() - start) / 1000 });
}
writeFileSync(join(out, "official.unminified.js"), bundled);
writeFileSync(join(out, "official.terser-nomangle.js"), (await minify(bundled, { module:true, compress:{passes:3}, mangle:false, format:{comments:false} })).code+'\n');
writeFileSync(join(out, "official.esbuild.js"), (await transform(bundled, { format:'esm',target:'esnext',minify:true,legalComments:'none' })).code);
try {
  const vite = await import(pathToFileURL(join(root,'node_modules/vite/dist/node/index.js')));
  if (typeof vite.minify === 'function') {
    for (const mangle of [true,false]) {
      const value=await vite.minify('official.js',bundled,{module:true,compress:true,mangle,codegen:{removeWhitespace:true,legalComments:'none'},sourcemap:false});
      if (value.errors?.length) throw new Error('Oxc rejected the upstream artifact');
      writeFileSync(join(out,`official.oxc-${mangle?'mangle':'nomangle'}.js`),value.code+'\n');
    }
  }
} catch (error) { if (error.code !== 'ERR_MODULE_NOT_FOUND') throw error; }
writeFileSync(join(out, "timing.json"), JSON.stringify({
  samples, medianSeconds: [...samples].sort((a,b) => a.wallSeconds-b.wallSeconds)[1].wallSeconds,
  scope: config.upstreamScope, entry: config.upstreamEntry,
  external, candidateExports,
  toolchain: { esbuild: "0.28.1", terser: "5.51.2" },
  protocol: "Three sequential warm-filesystem ESM bundle + Terser passes=3 builds. Includes writing the ESM file; excludes dependency installation, codec scoring and tests. First sample retained; no claim of a clean vendor release build.",
}, null, 2) + "\n");
