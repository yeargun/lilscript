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

const sourceDirectory = resolve(projectRoot, "src/compiler-dom");
const host = resolve(sourceDirectory, "host.js");
const artifact = resolve(projectRoot, "artifacts/compiler-dom.generated.js");
const facade = resolve(projectRoot, "packages/vuelil/compiler-dom.js");
const upstreamCandidate = resolve(
  projectRoot,
  "tests/compiler-dom-upstream.candidate.mjs",
);
const compilerCoreEvidence = JSON.parse(
  readFileSync(resolve(projectRoot, "compatibility/compiler-core.json"), "utf8"),
);
const coreExports = compilerCoreEvidence.runtimeExports;
const ownExports = [
  "DOMDirectiveTransforms",
  "DOMErrorCodes",
  "DOMErrorMessages",
  "DOMNodeTransforms",
  "TRANSITION",
  "TRANSITION_GROUP",
  "V_MODEL_CHECKBOX",
  "V_MODEL_DYNAMIC",
  "V_MODEL_RADIO",
  "V_MODEL_SELECT",
  "V_MODEL_TEXT",
  "V_ON_WITH_KEYS",
  "V_ON_WITH_MODIFIERS",
  "V_SHOW",
  "compile",
  "createDOMCompilerError",
  "parse",
  "parserOptions",
  "transformStyle",
];
const internalTestExports = [
  "decodeHtmlBrowser",
  "ignoreSideEffectTags",
  "isValidHTMLNesting",
  "StringifyThresholds",
  "stringifyStatic",
  "transformShow",
  "transformOn",
  "transformTransition",
  "transformVHtml",
  "transformModel",
  "transformVText",
];
const testCoreExports = coreExports.filter(
  name => !ownExports.includes(name) && !internalTestExports.includes(name),
);
const reflectedFunctions = new Map([
  ["compile", ["compile", 1]],
  ["createDOMCompilerError", ["createDOMCompilerError", 2]],
  ["decodeHtmlBrowser", ["decodeHtmlBrowser", 1]],
  ["isValidHTMLNesting", ["isValidHTMLNesting", 2]],
  ["parse", ["parse", 1]],
  ["stringifyStatic", ["stringifyStatic", 3]],
  ["transformStyle", ["transformStyle", 1]],
  ["transformOn", ["transformOn", 3]],
  ["transformModel", ["transformModel", 3]],
  ["transformShow", ["transformShow", 3]],
  ["transformVHtml", ["transformVHtml", 3]],
  ["transformVText", ["transformVText", 3]],
]);

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

// LilScript currently links extern declarations package-wide. Give each source
// module a private local alias while retaining the requested JavaScript export.
function isolateExternBindings(graph) {
  const hostAliases = new Map();
  const globalAliases = new Map();
  for (const path of lilFiles(graph)) {
    const original = readFileSync(path, "utf8");
    const moduleName = relative(graph, path)
      .split(sep)
      .join("_")
      .replace(/[^A-Za-z0-9_$]/gu, "_");
    const declarations = [...original.matchAll(
      /^extern\s+(?:JsValue|bool|int|string|void)\s+([A-Za-z_$][\w$]*)\s*(?=[(;=])/gmu,
    )].map(match => match[1]);
    if (declarations.length === 0) continue;

    const imports = [];
    const importedLocals = new Set();
    const masked = original.replace(
      /import\s+extern\s*\{([\s\S]*?)\}\s*from\s*"([^"]+)"\s*;/gu,
      (statement, bindings, specifier) => {
        const index = imports.length;
        imports.push({ bindings, specifier });
        for (const binding of bindings.split(",")) {
          const parts = binding.trim().split(/\s+as\s+/u);
          if (parts[0]) importedLocals.add(parts.at(-1));
        }
        return `__DOM_EXTERN_IMPORT_${index}__`;
      },
    );
    const aliases = new Map(
      declarations.map(name => [name, `__dom_${moduleName}_${name}`]),
    );
    let transformed = masked;
    for (const [name, alias] of aliases) {
      transformed = transformed.replace(
        new RegExp(`\\b${name}\\b`, "gu"),
        alias,
      );
      if (!importedLocals.has(name)) globalAliases.set(alias, name);
      if (name === "hostCreateHtmlDecoder") hostAliases.set(alias, name);
    }
    transformed = transformed.replace(
      /__DOM_EXTERN_IMPORT_(\d+)__/gu,
      (_placeholder, rawIndex) => {
        const { bindings, specifier } = imports[Number(rawIndex)];
        const rendered = bindings
          .split(",")
          .map(binding => {
            const parts = binding.trim().split(/\s+as\s+/u);
            const imported = parts[0];
            const local = parts.at(-1);
            const alias = aliases.get(local);
            if (!alias) return binding.trim();
            return `${imported} as ${alias}`;
          })
          .join(", ");
        return `import extern { ${rendered} } from ${JSON.stringify(specifier)};`;
      },
    );
    writeFileSync(path, transformed);
  }
  return { globalAliases, hostAliases };
}

function reflectFunctions(module) {
  return module.replace(/export\{([^}]*)\}\s*$/u, (statement, bindings) => {
    const definitions = bindings
      .split(",")
      .map(binding => {
        const [local, publicName = local] = binding.trim().split(/\s+as\s+/u);
        const reflection = reflectedFunctions.get(publicName);
        const memberDefinition = publicName === "DOMDirectiveTransforms"
          ? `Object.defineProperties(${local}.html,{name:{configurable:true,value:"transformVHtml"},length:{configurable:true,value:3}});Object.defineProperties(${local}.model,{name:{configurable:true,value:"transformModel"},length:{configurable:true,value:3}});Object.defineProperties(${local}.on,{name:{configurable:true,value:"transformOn"},length:{configurable:true,value:3}});Object.defineProperties(${local}.show,{name:{configurable:true,value:"transformShow"},length:{configurable:true,value:3}});Object.defineProperties(${local}.text,{name:{configurable:true,value:"transformVText"},length:{configurable:true,value:3}});`
          : "";
        if (!reflection) return memberDefinition;
        const [name, length] = reflection;
        return `${memberDefinition}Object.defineProperties(${local},{name:{configurable:true,value:${JSON.stringify(name)}},length:{configurable:true,value:${length}}});`;
      })
      .join("");
    return `${definitions}${statement}`;
  });
}

function assemble(compiledPath, dependencyPrefix, aliases) {
  let module = readFileSync(compiledPath, "utf8");
  for (const [alias, name] of [...aliases.globalAliases, ...aliases.hostAliases]) {
    module = module.replace(new RegExp(`\\b${alias}\\b`, "gu"), name);
  }
  module = module
    .replace(
      /import\s*\{[^}]*\bhostCreateHtmlDecoder\b[^}]*\}\s*from\s*["']\.\/host\.js["'];?/u,
      "",
    )
    .replace(
      /["'](?:\.\.\/){2,3}packages\/vuelil\/(compiler-core|shared)\.js["']/gu,
      (_specifier, name) => JSON.stringify(`${dependencyPrefix}${name}.js`),
    );
  if (/from\s*["']\.\/host\.js["']/u.test(module)) {
    throw new Error("failed to inline the compiler-dom host adapter");
  }
  return reflectFunctions(module);
}

function renderFacade() {
  return [
    'export * from "../../artifacts/compiler-dom.generated.js";',
    `export { ${coreExports.join(", ")} } from "./compiler-core.js";`,
    "",
  ].join("\n");
}

const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-compiler-dom-"));
const graph = resolve(temporary, "src/compiler-dom");
const compiled = resolve(temporary, "index.js");
const compiledTest = resolve(temporary, "test.js");

try {
  mkdirSync(resolve(temporary, "src"), { recursive: true });
  mkdirSync(resolve(temporary, "packages/vuelil"), { recursive: true });
  cpSync(sourceDirectory, graph, { recursive: true });
  copyFileSync(
    resolve(projectRoot, "packages/vuelil/compiler-core.js"),
    resolve(temporary, "packages/vuelil/compiler-core.js"),
  );
  copyFileSync(
    resolve(projectRoot, "packages/vuelil/shared.js"),
    resolve(temporary, "packages/vuelil/shared.js"),
  );
  writeFileSync(
    resolve(graph, "test.lil"),
    [
      `import { ${ownExports.join(", ")} } from "./index";`,
      'import { decodeHtmlBrowser } from "./decodeHtmlBrowser";',
      'import { isValidHTMLNesting } from "./htmlNesting";',
      'import { transformShow } from "./transforms/vShow";',
      'import { transformOn } from "./transforms/vOn";',
      'import { transformTransition } from "./transforms/Transition";',
      'import { ignoreSideEffectTags } from "./transforms/ignoreSideEffectTags";',
      'import { StringifyThresholds, stringifyStatic } from "./transforms/stringifyStatic";',
      'import { transformVHtml } from "./transforms/vHtml";',
      'import { transformModel } from "./transforms/vModel";',
      'import { transformVText } from "./transforms/vText";',
      `export { ${[
        ...ownExports,
        ...internalTestExports,
      ].join(", ")} };`,
      "",
    ].join("\n"),
  );
  const aliases = isolateExternBindings(graph);
  runCompiler(resolve(graph, "index.lil"), compiled);
  runCompiler(resolve(graph, "test.lil"), compiledTest);

  const hostModule = readFileSync(host, "utf8").replaceAll(
    "export function ",
    "function ",
  );
  const banner = "// Generated from the complete compiler-dom source graph.\n";
  mkdirSync(resolve(projectRoot, "artifacts"), { recursive: true });
  mkdirSync(resolve(projectRoot, "packages/vuelil"), { recursive: true });
  writeFileSync(
    artifact,
    `${banner}${hostModule}\n${assemble(compiled, "../packages/vuelil/", aliases)}`,
  );
  writeFileSync(facade, renderFacade());
  writeFileSync(
    upstreamCandidate,
    `${banner}${hostModule}\n${assemble(compiledTest, "../packages/vuelil/", aliases)}\n` +
      `export { ${testCoreExports.join(", ")} } from "../packages/vuelil/compiler-core.js";\n`,
  );

  const runtime = await import(`${facade}?build=${Date.now()}`);
  const missing = [...ownExports, ...coreExports].filter(name => !(name in runtime));
  if (missing.length > 0) {
    throw new Error(`compiler-dom facade is missing exports: ${missing.join(", ")}`);
  }
  console.log(JSON.stringify({
    artifact,
    facade,
    exports: Object.keys(runtime).length,
  }));
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
