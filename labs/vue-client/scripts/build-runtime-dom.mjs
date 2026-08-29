import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { compilerPath, projectRoot, repositoryRoot } from "../tooling/compiler-path.mjs";

const source = resolve(projectRoot, "src/runtime-dom/index.lil");
const host = resolve(projectRoot, "src/runtime-dom/host.js");
const output = resolve(projectRoot, "packages/vuelil/runtime-dom.js");
const testOutput = resolve(projectRoot, "tests/runtime-dom-upstream.candidate.mjs");
const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-runtime-dom-"));
const compiled = resolve(temporary, "index.js");

const publicCompiledExports = new Set([
  "initDirectivesForSSR",
  "nodeOps",
  "patchProp",
  "vModelCheckbox",
  "vModelDynamic",
  "vModelRadio",
  "vModelSelect",
  "vModelText",
  "vShow",
  "withKeys",
  "withModifiers",
]);

const functionLengths = new Map([
  ["addEventListener", 3],
  ["initDirectivesForSSR", 0],
  ["initVModelForSSR", 0],
  ["initVShowForSSR", 0],
  ["patchAttr", 4],
  ["patchClass", 3],
  ["patchDOMProp", 4],
  ["patchEvent", 4],
  ["patchProp", 6],
  ["patchStyle", 3],
  ["removeEventListener", 3],
  ["unsafeToTrustedHTML", 1],
  ["withKeys", 2],
  ["withModifiers", 2],
]);

const objectMethods = {
  nodeOps: {
    insert: 3,
    remove: 1,
    createElement: 4,
    createText: 1,
    createComment: 1,
    setText: 2,
    setElementText: 2,
    parentNode: 1,
    nextSibling: 1,
    querySelector: 1,
    setScopeId: 2,
    insertStaticContent: 6,
  },
  vShow: { beforeMount: 3, mounted: 3, updated: 3, beforeUnmount: 2 },
  vModelText: { created: 3, mounted: 2, beforeUpdate: 3 },
  vModelCheckbox: { created: 3, mounted: ["setChecked", 3], beforeUpdate: 3 },
  vModelRadio: { created: 3, beforeUpdate: 3 },
  vModelSelect: { created: 3, mounted: 2, beforeUpdate: 3, updated: 2 },
  vModelDynamic: { created: 3, mounted: 3, beforeUpdate: 4, updated: 4 },
};

function parseExports(exports) {
  return exports.split(",").map(entry => {
    const [local, publicName = local] = entry.trim().split(/\s+as\s+/);
    return { local, publicName };
  });
}

function descriptor(target, name, length) {
  return `Object.defineProperties(${target},{name:{configurable:true,value:${JSON.stringify(name)}},length:{configurable:true,value:${length}}});`;
}

function prepareModule(module, publicOnly, corePrefix) {
  return module.replace(/export\{([^}]*)\}\s*$/, (_statement, exports) => {
    const entries = parseExports(exports);
    const definitions = [];
    for (const { local, publicName } of entries) {
      if (functionLengths.has(publicName)) {
        definitions.push(descriptor(local, publicName, functionLengths.get(publicName)));
      }
      const methods = objectMethods[publicName];
      if (methods) {
        for (const [method, reflection] of Object.entries(methods)) {
          const [name, length] = Array.isArray(reflection) ? reflection : [method, reflection];
          definitions.push(descriptor(`${local}.${method}`, name, length));
        }
      }
    }
    const kept = entries.filter(({ publicName }) => !publicOnly || publicCompiledExports.has(publicName));
    const statement = `export{${kept.map(({ local, publicName }) =>
      local === publicName ? local : `${local} as ${publicName}`).join(",")}};`;
    return `${definitions.join("")}${statement}export*from${JSON.stringify(`${corePrefix}runtime-core.js`)};`;
  });
}

try {
  const result = spawnSync(
    compilerPath(),
    [
      source,
      "--target", "js-module",
      "--mode", "development",
      "--config", resolve(repositoryRoot, "tests/config/no-optimization-no-peephole.toml"),
      "-o", compiled,
    ],
    { cwd: projectRoot, encoding: "utf8", env: process.env },
  );
  if (result.status !== 0) throw new Error(`${result.stdout ?? ""}${result.stderr ?? ""}`);

  const hostModule = readFileSync(host, "utf8").replaceAll("export function ", "function ");
  let compiledModule = readFileSync(compiled, "utf8")
    .replaceAll('"../../packages/vuelil/runtime-core.js"', '"./runtime-core.js"')
    .replaceAll('"../../packages/vuelil/shared.js"', '"./shared.js"')
    .replace(/import\{[^;]*\}from["']\.\/host\.js["'];?/, "");
  if (/from["']\.\/host\.js["']/.test(compiledModule)) {
    throw new Error("failed to inline the runtime-dom host adapter");
  }

  const banner = "// Generated from src/runtime-dom/index.lil and its measured DOM host adapter.\n";
  mkdirSync(resolve(projectRoot, "packages/vuelil"), { recursive: true });
  writeFileSync(output, `${banner}${hostModule}\n${prepareModule(compiledModule, true, "./")}\n`);

  const testModule = compiledModule
    .replaceAll('"./runtime-core.js"', '"../packages/vuelil/runtime-core.js"')
    .replaceAll('"./shared.js"', '"../packages/vuelil/shared.js"');
  writeFileSync(
    testOutput,
    `${banner}${hostModule}\n${prepareModule(testModule, false, "../packages/vuelil/")}\n`,
  );

  const runtime = await import(`../packages/vuelil/runtime-dom.js?build=${Date.now()}`);
  console.log(JSON.stringify({
    output,
    exports: Object.keys(runtime).sort(),
    bytes: readFileSync(output).byteLength,
  }));
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
