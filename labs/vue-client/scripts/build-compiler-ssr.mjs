import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { relative, resolve, sep } from "node:path";
import { compilerPath, projectRoot } from "../tooling/compiler-path.mjs";

const sourceDirectory = resolve(projectRoot, "src/compiler-ssr");
const artifact = resolve(projectRoot, "artifacts/compiler-ssr.generated.js");
const facade = resolve(projectRoot, "packages/vuelil/compiler-ssr.js");
const upstreamCandidate = resolve(
  projectRoot,
  "tests/compiler-ssr-upstream.candidate.mjs",
);
const runtimeHelpers = [
  "SSR_GET_DIRECTIVE_PROPS",
  "SSR_GET_DYNAMIC_MODEL_PROPS",
  "SSR_INCLUDE_BOOLEAN_ATTR",
  "SSR_INTERPOLATE",
  "SSR_LOOSE_CONTAIN",
  "SSR_LOOSE_EQUAL",
  "SSR_RENDER_ATTR",
  "SSR_RENDER_ATTRS",
  "SSR_RENDER_CLASS",
  "SSR_RENDER_COMPONENT",
  "SSR_RENDER_DYNAMIC_ATTR",
  "SSR_RENDER_DYNAMIC_MODEL",
  "SSR_RENDER_LIST",
  "SSR_RENDER_SLOT",
  "SSR_RENDER_SLOT_INNER",
  "SSR_RENDER_STYLE",
  "SSR_RENDER_SUSPENSE",
  "SSR_RENDER_TELEPORT",
  "SSR_RENDER_VNODE",
  "ssrHelpers",
];

function runCompiler(input, output) {
  const result = spawnSync(
    compilerPath(),
    [
      input,
      "--target",
      "js-module",
      "--mode",
      "development",
      "--jobs",
      "1",
      "--codec-jobs",
      "1",
      "-o",
      output,
    ],
    { cwd: projectRoot, encoding: "utf8", env: process.env },
  );
  if (result.status !== 0) {
    throw new Error(`${result.stdout ?? ""}${result.stderr ?? ""}`);
  }
}

function lilFiles(root) {
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = resolve(root, entry.name);
    if (entry.isDirectory()) files.push(...lilFiles(path));
    else if (entry.name.endsWith(".lil")) files.push(path);
  }
  return files;
}

// Extern declarations currently link package-wide, so isolate each owner's
// local import aliases before compiling the graph.
function isolateExternBindings(graph) {
  const aliases = new Map();
  for (const path of lilFiles(graph)) {
    const original = readFileSync(path, "utf8");
    const moduleName = relative(graph, path)
      .split(sep)
      .join("_")
      .replace(/[^A-Za-z0-9_$]/gu, "_");
    const imports = [];
    const importedLocals = new Set();
    const masked = original.replace(
      /import\s+extern\s*\{([\s\S]*?)\}\s*from\s*"([^"]+)"\s*;/gu,
      (_statement, bindings, specifier) => {
        const index = imports.length;
        imports.push({ bindings, specifier });
        for (const binding of bindings.split(",")) {
          const parts = binding.trim().split(/\s+as\s+/u);
          if (parts[0]) importedLocals.add(parts.at(-1));
        }
        return `__SSR_EXTERN_IMPORT_${index}__`;
      },
    );
    const declarations = [...masked.matchAll(
      /^extern\s+(?:JsValue|bool|int|string|void)\s+([A-Za-z_$][\w$]*)\s*(?=[(;=])/gmu,
    )].map(match => match[1]);
    const localAliases = new Map(
      declarations.map(name => [name, `__ssr_${moduleName}_${name}`]),
    );
    let transformed = masked;
    for (const [name, alias] of localAliases) {
      transformed = transformed.replace(new RegExp(`\\b${name}\\b`, "gu"), alias);
      if (!importedLocals.has(name)) aliases.set(alias, name);
    }
    transformed = transformed.replace(
      /__SSR_EXTERN_IMPORT_(\d+)__/gu,
      (_placeholder, rawIndex) => {
        const { bindings, specifier } = imports[Number(rawIndex)];
        const rendered = bindings
          .split(",")
          .map(binding => {
            const parts = binding.trim().split(/\s+as\s+/u);
            const imported = parts[0];
            const local = parts.at(-1);
            const alias = localAliases.get(local);
            return alias ? `${imported} as ${alias}` : binding.trim();
          })
          .join(", ");
        return `import extern { ${rendered} } from ${JSON.stringify(specifier)};`;
      },
    );
    writeFileSync(path, transformed);
  }
  return aliases;
}

function assemble(compiledPath, dependencyPrefix, aliases) {
  let module = readFileSync(compiledPath, "utf8");
  for (const [alias, name] of aliases) {
    module = module.replace(new RegExp(`\\b${alias}\\b`, "gu"), name);
  }
  return module.replace(
    /["'](?:\.\.\/){2,3}packages\/vuelil\/(compiler-dom|shared)\.js["']/gu,
    (_specifier, name) => JSON.stringify(`${dependencyPrefix}${name}.js`),
  );
}

function reflectCompile(module) {
  return module.replace(/export\{([^}]*)\}\s*$/u, (statement, bindings) => {
    const compileBinding = bindings
      .split(",")
      .map(binding => binding.trim().split(/\s+as\s+/u))
      .find(([_local, publicName]) => (publicName ?? _local) === "compile");
    if (!compileBinding) throw new Error("compiled SSR module does not export compile");
    return `Object.defineProperties(${compileBinding[0]},{name:{configurable:true,value:"compile"},length:{configurable:true,value:1}});${statement}`;
  });
}

const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-compiler-ssr-"));
const graph = resolve(temporary, "src/compiler-ssr");
const compiled = resolve(temporary, "index.js");
const compiledTest = resolve(temporary, "test.js");

try {
  mkdirSync(resolve(temporary, "src"), { recursive: true });
  mkdirSync(resolve(temporary, "packages/vuelil"), { recursive: true });
  cpSync(sourceDirectory, graph, { recursive: true });
  copyFileSync(
    resolve(projectRoot, "packages/vuelil/compiler-dom.js"),
    resolve(temporary, "packages/vuelil/compiler-dom.js"),
  );
  copyFileSync(
    resolve(projectRoot, "packages/vuelil/shared.js"),
    resolve(temporary, "packages/vuelil/shared.js"),
  );
  writeFileSync(
    resolve(graph, "test.lil"),
    [
      'import { compile } from "./index";',
      `import { ${runtimeHelpers.join(", ")} } from "./runtimeHelpers";`,
      `export { compile, ${runtimeHelpers.join(", ")} };`,
      "",
    ].join("\n"),
  );
  const aliases = isolateExternBindings(graph);
  runCompiler(resolve(graph, "index.lil"), compiled);
  runCompiler(resolve(graph, "test.lil"), compiledTest);

  const banner = "// Generated from the complete compiler-ssr LilScript source graph.\n";
  mkdirSync(resolve(projectRoot, "artifacts"), { recursive: true });
  mkdirSync(resolve(projectRoot, "packages/vuelil"), { recursive: true });
  writeFileSync(
    artifact,
    `${banner}${reflectCompile(assemble(compiled, "../packages/vuelil/", aliases))}`,
  );
  writeFileSync(
    facade,
    'export { compile } from "../../artifacts/compiler-ssr.generated.js";\n',
  );
  writeFileSync(
    upstreamCandidate,
    `${banner}${reflectCompile(assemble(compiledTest, "../packages/vuelil/", aliases))}`,
  );

  const runtime = await import(`${facade}?build=${Date.now()}`);
  if (Object.keys(runtime).join(",") !== "compile") {
    throw new Error(`compiler-ssr facade exports ${Object.keys(runtime).join(", ")}`);
  }
  console.log(JSON.stringify({ artifact, facade, exports: Object.keys(runtime) }));
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
