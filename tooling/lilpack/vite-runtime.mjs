import { spawnSync } from "node:child_process";
import path from "node:path";
import { pathToFileURL } from "node:url";

const options = parseArguments(process.argv.slice(2));
const vite = await import(pathToFileURL(options.vite).href);
const plugin = lilscriptPlugin(options);

if (options.command === "dev") {
  const server = await vite.createServer({
    root: options.root,
    base: options.base,
    cacheDir: path.join(options.root, ".lilpack/cache"),
    appType: "spa",
    configFile: false,
    clearScreen: false,
    plugins: [plugin],
    server: {
      host: options.host,
      port: options.port,
      strictPort: true,
      open: options.open,
    },
  });
  await server.listen();
  console.log("\n  Lilpack dev server");
  server.printUrls();
} else {
  await vite.build({
    root: options.root,
    base: options.base,
    cacheDir: path.join(options.root, ".lilpack/cache"),
    appType: "spa",
    configFile: false,
    clearScreen: false,
    plugins: [plugin],
    build: {
      outDir: options.outDir,
      emptyOutDir: true,
      manifest: "lilpack.manifest.json",
      modulePreload: { polyfill: false },
      minify: options.minify ? "oxc" : false,
      sourcemap: options.sourcemap,
      // Lilscript emits modern ESM. `esnext` keeps Vite 8 on its Rolldown/Oxc
      // path instead of pulling the deprecated optional esbuild transpiler in.
      target: "esnext",
      rollupOptions: {
        input: path.join(options.root, "index.html"),
      },
    },
  });
}

function lilscriptPlugin(options) {
  const dependencyOwners = new Map();
  const ownerDependencies = new Map();
  const entryUrl = `/${path.relative(options.root, options.entry).split(path.sep).join("/")}`;
  const entryRequest = options.command === "dev" ? `${entryUrl}?import` : entryUrl;

  return {
    name: "lilpack:lilscript",
    enforce: "pre",

    transformIndexHtml: {
      order: "pre",
      handler(html) {
        if (html.includes(entryRequest)) return html;
        if (html.includes(entryUrl)) {
          return options.command === "dev"
            ? html.replaceAll(entryUrl, entryRequest)
            : html;
        }
        return {
          html,
          tags: [
            {
              tag: "script",
              attrs: { type: "module", src: entryRequest },
              injectTo: "body",
            },
          ],
        };
      },
    },

    transform(_source, id) {
      const file = cleanModuleId(id);
      if (path.extname(file) !== ".lil") return null;

      const dependencies = compilerDependencies(file, options);
      trackDependencies(file, dependencies, ownerDependencies, dependencyOwners);
      for (const dependency of dependencies) this.addWatchFile(dependency);

      let code = compileLilscript(file, options);
      if (options.command === "dev") code = addHotBoundary(code);
      return { code, map: null };
    },

    handleHotUpdate(context) {
      const changed = normalizeFile(context.file);
      const owners = dependencyOwners.get(changed);
      if (!owners || owners.size === 0) return;
      if (owners.size === 1 && owners.has(changed)) return;

      const affected = new Set(context.modules);
      for (const owner of owners) {
        const modules = context.server.moduleGraph.getModulesByFile(owner);
        if (!modules) continue;
        for (const module of modules) {
          context.server.moduleGraph.invalidateModule(
            module,
            new Set(),
            context.timestamp,
            true,
          );
          affected.add(module);
        }
      }
      return [...affected];
    },
  };
}

function compileLilscript(file, options) {
  const result = invokeCompiler(
    file,
    [
      "--target",
      "js-module",
      "--mode",
      options.command === "dev" ? "development" : "production",
      "--delegate-bundling",
    ],
    options,
  );
  return result.stdout;
}

function compilerDependencies(file, options) {
  const result = invokeCompiler(
    file,
    [
      "--target",
      "js-module",
      "--mode",
      "development",
      "--delegate-bundling",
      "--print-dependencies",
    ],
    options,
  );
  try {
    const metadata = JSON.parse(result.stdout);
    if (metadata.version !== 1 || !Array.isArray(metadata.files)) {
      throw new Error("unsupported metadata shape");
    }
    return metadata.files.map(normalizeFile);
  } catch (error) {
    throw new Error(`Lilscript returned invalid dependency metadata: ${error.message}`);
  }
}

function invokeCompiler(file, args, options) {
  const compilerArgs = [file, ...args];
  if (options.config) compilerArgs.push("--config", options.config);
  const result = spawnSync(options.compiler, compilerArgs, {
    cwd: options.root,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error) {
    throw new Error(`failed to run Lilscript compiler: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error((result.stderr || "Lilscript compilation failed").trim());
  }
  return result;
}

function trackDependencies(owner, dependencies, ownerDependencies, dependencyOwners) {
  const normalizedOwner = normalizeFile(owner);
  for (const dependency of ownerDependencies.get(normalizedOwner) ?? []) {
    const owners = dependencyOwners.get(dependency);
    owners?.delete(normalizedOwner);
    if (owners?.size === 0) dependencyOwners.delete(dependency);
  }
  const normalizedDependencies = new Set(dependencies.map(normalizeFile));
  ownerDependencies.set(normalizedOwner, normalizedDependencies);
  for (const dependency of normalizedDependencies) {
    let owners = dependencyOwners.get(dependency);
    if (!owners) dependencyOwners.set(dependency, (owners = new Set()));
    owners.add(normalizedOwner);
  }
}

function addHotBoundary(code) {
  const hotAccept = exportedLocal(code, "hotAccept");
  if (!hotAccept) return code;
  const hotDispose = exportedLocal(code, "hotDispose");
  const dispose = hotDispose
    ? `import.meta.hot.dispose(()=>${hotDispose}());`
    : "";
  return `${code}\nif(import.meta.hot){${dispose}import.meta.hot.accept(module=>module?.hotAccept?.());}`;
}

function exportedLocal(code, publicName) {
  const exports = [...code.matchAll(/export\{([^}]*)\}/gu)].at(-1)?.[1];
  if (!exports) return null;
  for (const item of exports.split(",")) {
    const match = item.trim().match(/^([A-Za-z_$][\w$]*)(?:\s+as\s+([A-Za-z_$][\w$]*))?$/u);
    if (match && (match[2] ?? match[1]) === publicName) return match[1];
  }
  return null;
}

function cleanModuleId(id) {
  return normalizeFile(id.split("?", 1)[0].replace(/^\0/u, ""));
}

function normalizeFile(file) {
  return path.resolve(file);
}

function parseArguments(argv) {
  const values = new Map();
  const flags = new Set();
  let command;
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "dev" || argument === "build") {
      command = argument;
    } else if (["--open", "--sourcemap", "--no-minify"].includes(argument)) {
      flags.add(argument);
    } else if (argument.startsWith("--")) {
      const value = argv[index + 1];
      if (value === undefined) throw new Error(`missing value for ${argument}`);
      values.set(argument, value);
      index += 1;
    } else {
      throw new Error(`unexpected Lilpack engine argument: ${argument}`);
    }
  }
  if (!command) throw new Error("missing Lilpack command");
  for (const required of ["--entry", "--root", "--compiler", "--vite"]) {
    if (!values.has(required)) throw new Error(`missing ${required}`);
  }
  return {
    command,
    entry: path.resolve(values.get("--entry")),
    root: path.resolve(values.get("--root")),
    compiler: path.resolve(values.get("--compiler")),
    vite: path.resolve(values.get("--vite")),
    config: values.get("--config"),
    base: values.get("--base") ?? "/",
    host: values.get("--host") ?? "127.0.0.1",
    port: Number(values.get("--port") ?? "5173"),
    outDir: values.get("--out-dir"),
    open: flags.has("--open"),
    sourcemap: flags.has("--sourcemap"),
    minify: !flags.has("--no-minify"),
  };
}
