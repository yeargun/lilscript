import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { build as esbuild, transform as esbuildTransform } from "esbuild";
import { canonicalCodecMeasurementsForFiles } from "../../codec-contract.mjs";
import { coreEsm, labRoot, lilPath, monacoPath, portsRoot } from "./file-map.mjs";
import { runCompiler } from "./measure-pairs.mjs";
import { writeKeepFile } from "./catalog-keep.mjs";

function folderOf(rel) {
  const parts = rel.split("/");
  if (parts.length < 2) return parts[0] || rel;
  if (parts[0] === "editor" && parts[1] === "contrib") {
    return parts.slice(0, 3).join("/");
  }
  return parts.slice(0, 2).join("/");
}

function isHeavyLil(src) {
  return /from ["'][^"']*(?:editor\/view|editor\/standalone|text-model|monaco-api|editor\/commands|contrib\/runtime|contrib\/popular)["']/.test(src);
}

const relativeExternalPlugin = {
  name: "monaco-relative-externals",
  setup(build) {
    build.onResolve({ filter: /^\./ }, (args) => {
      if (args.kind === "entry-point") return undefined;
      return { path: args.path, external: true };
    });
  },
};

async function minifyEsbuild(source, filename) {
  const result = await esbuildTransform(source, {
    sourcefile: filename,
    loader: "js",
    format: "esm",
    target: "esnext",
    minify: true,
    legalComments: "none",
  });
  if (!result.code) {
    return "export{};";
  }
  return result.code;
}

async function bundleJsFile(rel, outFile) {
  await esbuild({
    absWorkingDir: coreEsm,
    entryPoints: [monacoPath(rel)],
    outfile: outFile,
    bundle: true,
    format: "esm",
    platform: "neutral",
    minify: false,
    write: true,
    plugins: [relativeExternalPlugin],
    logOverride: { "import-is-undefined": "silent" },
  });
  return readFileSync(outFile, "utf8");
}

const keepRoot = join(labRoot, "build/monaco-layers/catalog/keep");

function compileLilFile(lilRel, outDir) {
  mkdirSync(outDir, { recursive: true });
  const compiledPath = join(outDir, "lilscript.raw.js");
  const lilAbs = lilPath(lilRel);
  const src = readFileSync(lilAbs, "utf8");
  const keepAbs = join(keepRoot, lilRel.replace(/\.lil$/, ".keep.lil"));
  const keep = writeKeepFile(src, lilAbs, keepAbs);
  try {
    runCompiler(keep || lilAbs, compiledPath);
  } catch (err) {
    if (!keep) throw err;
    runCompiler(lilAbs, compiledPath);
  }
  return compiledPath;
}

function writeCatalogPackEntry(artifacts, packEntryPath) {
  const ok = artifacts.filter((item) => item.ok && item.artifact);
  const modules = ok.map((item) => JSON.stringify(readFileSync(item.artifact, "utf8")));
  const body = `globalThis.__lilMonacoCatalog = [${modules.join(",")}];\n`;
  mkdirSync(dirname(packEntryPath), { recursive: true });
  writeFileSync(packEntryPath, body);
  return packEntryPath;
}

function sumSizes(rows, key) {
  const js = { raw: 0, gzip: 0, brotli: 0 };
  const lil = { raw: 0, gzip: 0, brotli: 0 };
  let lilCount = 0;
  for (const row of rows) {
    if (row.js) {
      js.raw += row.js.raw;
      js.gzip += row.js.gzip;
      js.brotli += row.js.brotli;
    }
    if (row.lil && row.lil.unique !== false) {
      lil.raw += row.lil.raw;
      lil.gzip += row.lil.gzip;
      lil.brotli += row.lil.brotli;
      lilCount += 1;
    }
  }
  return { key, files: rows.length, scoredLil: lilCount, js, lil: lilCount ? lil : null };
}

export async function measureCatalog(catalog, outRoot) {
  mkdirSync(outRoot, { recursive: true });
  const jsDir = join(outRoot, "js");
  const lilDir = join(outRoot, "lil");
  mkdirSync(jsDir, { recursive: true });
  mkdirSync(lilDir, { recursive: true });

  const jsArtifacts = [];
  let i = 0;
  for (const row of catalog.files) {
    i += 1;
    if (i % 100 === 0 || i === catalog.files.length) {
      console.log(`  catalog js ${i}/${catalog.files.length}`);
    }
    const id = row.monaco.replace(/[^\w./-]+/g, "_").split("/").join("__");
    const bundled = join(jsDir, `${id}.bundle.js`);
    const minifiedPath = join(jsDir, `${id}.min.js`);
    try {
      const bundledSource = await bundleJsFile(row.monaco, bundled);
      const minified = await minifyEsbuild(bundledSource, row.monaco);
      writeFileSync(minifiedPath, minified);
      jsArtifacts.push({ row, minifiedPath, ok: true });
    } catch (err) {
      console.error(`  js ${row.monaco}: ${err.message}`);
      jsArtifacts.push({ row, minifiedPath: null, ok: false, error: err.message });
    }
  }

  const jsMeasured = canonicalCodecMeasurementsForFiles(
    jsArtifacts.filter((item) => item.ok).map((item) => item.minifiedPath),
    "monaco-editor-core catalog JS",
  );
  const jsByPath = new Map();
  let m = 0;
  for (const item of jsArtifacts) {
    if (!item.ok) continue;
    const sizes = jsMeasured[m];
    jsByPath.set(item.row.monaco, { raw: sizes.raw, gzip: sizes.gzip, brotli: sizes.brotli });
    m += 1;
  }

  const lilArtifacts = [];
  for (const row of catalog.files) {
    if (row.status !== "ported") continue;
    const src = readFileSync(join(portsRoot, row.lil), "utf8");
    if (isHeavyLil(src)) continue;
    const id = row.monaco.replace(/[^\w./-]+/g, "_").split("/").join("__");
    try {
      const artifact = await compileLilFile(row.lil, join(lilDir, id));
      lilArtifacts.push({ row, artifact, ok: true });
    } catch (err) {
      console.error(`  lil ${row.lil}: ${err.message.split("\n")[0]}`);
      lilArtifacts.push({ row, artifact: null, ok: false, error: err.message });
    }
  }

  const uniqueImpls = [];
  const seenImpl = new Set(lilArtifacts.filter((item) => item.ok).map((item) => item.row.lil));
  for (const row of catalog.files) {
    if (row.status !== "shim" || !row.impl) continue;
    if (seenImpl.has(row.impl)) continue;
    seenImpl.add(row.impl);
    if (isHeavyLil(readFileSync(join(portsRoot, row.impl), "utf8"))) continue;
    const id = `impl__${row.impl.replace(/[^\w./-]+/g, "_").split("/").join("__")}`;
    try {
      const artifact = await compileLilFile(row.impl, join(lilDir, id));
      uniqueImpls.push({ impl: row.impl, artifact, ok: true });
    } catch (err) {
      console.error(`  lil impl ${row.impl}: ${err.message.split("\n")[0]}`);
    }
  }

  const lilMeasured = lilArtifacts.filter((item) => item.ok).length
    ? canonicalCodecMeasurementsForFiles(
      lilArtifacts.filter((item) => item.ok).map((item) => item.artifact),
      "monaco-editor-core catalog Lil",
    )
    : [];
  const lilByMonaco = new Map();
  let li = 0;
  for (const item of lilArtifacts) {
    if (!item.ok) continue;
    const sizes = lilMeasured[li];
    lilByMonaco.set(item.row.monaco, { raw: sizes.raw, gzip: sizes.gzip, brotli: sizes.brotli, lil: item.row.lil });
    li += 1;
  }

  const implMeasured = uniqueImpls.filter((item) => item.ok).length
    ? canonicalCodecMeasurementsForFiles(
      uniqueImpls.filter((item) => item.ok).map((item) => item.artifact),
      "monaco-editor-core catalog Lil impls",
    )
    : [];
  const lilByImpl = new Map();
  let ii = 0;
  for (const item of uniqueImpls) {
    if (!item.ok) continue;
    const sizes = implMeasured[ii];
    lilByImpl.set(item.impl, { raw: sizes.raw, gzip: sizes.gzip, brotli: sizes.brotli, lil: item.impl });
    ii += 1;
  }

  const claimedImpl = new Set();
  const files = catalog.files.map((row) => {
    const js = jsByPath.get(row.monaco) ?? null;
    const direct = lilByMonaco.get(row.monaco) ?? null;
    const viaImpl = row.impl ? lilByImpl.get(row.impl) : null;
    let lil = null;
    if (direct) {
      lil = { path: direct.lil, raw: direct.raw, gzip: direct.gzip, brotli: direct.brotli, unique: true };
    } else if (viaImpl) {
      const first = !claimedImpl.has(row.impl);
      if (first) claimedImpl.add(row.impl);
      lil = { path: viaImpl.lil, raw: viaImpl.raw, gzip: viaImpl.gzip, brotli: viaImpl.brotli, unique: first };
    }
    return {
      monaco: row.monaco,
      lilPath: row.lil,
      impl: row.impl || undefined,
      status: row.status,
      folder: folderOf(row.monaco),
      js,
      lil,
    };
  });

  const uniqueLilRows = files.filter((row) => row.lil && row.lil.unique);
  const folders = [];
  const grouped = new Map();
  for (const row of files) {
    const list = grouped.get(row.folder) ?? [];
    list.push(row);
    grouped.set(row.folder, list);
  }
  for (const [key, rows] of grouped) {
    const folder = sumSizes(rows, key);
    folder.lil = sumSizes(rows.filter((row) => row.lil && row.lil.unique), key).lil;
    folder.scoredLil = rows.filter((row) => row.lil && row.lil.unique).length;
    folders.push(folder);
  }
  folders.sort((a, b) => b.js.brotli - a.js.brotli);

  const totals = sumSizes(files, "monaco-editor-core");
  totals.lil = sumSizes(uniqueLilRows, "monaco-editor-core").lil;
  totals.scoredLil = uniqueLilRows.length;

  const packEntry = writeCatalogPackEntry(
    [...lilArtifacts, ...uniqueImpls.map((item) => ({ ok: item.ok, artifact: item.artifact }))],
    join(labRoot, "build/monaco-layers/catalog-pack-entry.js"),
  );

  return {
    protocol: {
      js: "each monaco-editor-core ESM file; other monaco imports external; esbuild minify; lilscript-codec gzip-9 / brotli-11",
      lil: "independently compiled .lil implementations with js-module keepers so exported class methods stay in the artifact (source export class is type-only and would otherwise DCE to empty)",
    },
    totals,
    folders,
    files,
    scoredLil: uniqueLilRows.length,
    displayedLil: files.filter((row) => row.lil).length,
    missingJs: files.filter((row) => !row.js).length,
    packEntry,
  };
}
