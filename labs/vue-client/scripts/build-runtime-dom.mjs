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
import { resolve } from "node:path";
import { compilerPath, projectRoot, repositoryRoot } from "../tooling/compiler-path.mjs";

const source = resolve(projectRoot, "src/runtime-dom/index.lil");
const host = resolve(projectRoot, "src/runtime-dom/host.js");
const output = resolve(projectRoot, "packages/vuelil/runtime-dom.js");
const testOutput = resolve(projectRoot, "tests/runtime-dom-upstream.candidate.mjs");
const browserOutput = resolve(projectRoot, "tests/vue-runtime-dom-browser.candidate.mjs");
const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-runtime-dom-"));
const compiled = resolve(temporary, "index.js");
const browserCompiled = resolve(temporary, "browser.js");
const projectCompiled = resolve(temporary, "project.js");
const productionPackageCompiled = resolve(temporary, "production-package.js");
const projectVariant = process.env.VUELIL_PROJECT_VARIANT;
const projectDirectory = projectVariant
  ? resolve(projectRoot, `packages/vuelil/production/${projectVariant}`)
  : resolve(projectRoot, "packages/vuelil");
const projectOutput = resolve(
  projectDirectory,
  projectVariant ? "runtime-dom.js" : "runtime-dom.project.generated.js",
);
const projectRuntimeCoreInput = resolve(
  projectDirectory,
  projectVariant ? "runtime-core.js" : "runtime-core.project.generated.js",
);
const projectSharedInput = resolve(
  projectDirectory,
  projectVariant ? "shared.js" : "shared.project.generated.js",
);
const projectVueOutput = projectVariant
  ? resolve(projectDirectory, "vue.runtime.js")
  : null;
const productionDirectory = resolve(projectRoot, "packages/vuelil/production");
const productionOutput = resolve(productionDirectory, "runtime-dom.js");
const defaultProjectExports = [
  "Fragment",
  "computed",
  "createApp",
  "createBlock",
  "createElementBlock",
  "createElementVNode",
  "createTextVNode",
  "createVNode",
  "defineComponent",
  "h",
  "inject",
  "nextTick",
  "normalizeClass",
  "openBlock",
  "popScopeId",
  "provide",
  "pushScopeId",
  "reactive",
  "ref",
  "registerRuntimeCompiler",
  "renderList",
  "resolveComponent",
  "toDisplayString",
];
const projectExports = process.env.VUELIL_PROJECT_EXPORTS
  ? process.env.VUELIL_PROJECT_EXPORTS.split(",").filter(Boolean)
  : defaultProjectExports;

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
const internalTestExports = new Set(["mathmlNS", "patchEvent", "vtcKey", "xlinkNS"]);

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

function exportedNames(module, publicOnly) {
  const match = module.match(/export\{([^}]*)\}\s*$/u);
  if (!match) throw new Error("compiled runtime-dom module has no export clause");
  return parseExports(match[1])
    .filter(({ publicName }) => !publicOnly || !internalTestExports.has(publicName))
    .map(({ publicName }) => publicName);
}

function descriptor(target, name, length) {
  return `Object.defineProperties(${target},{name:{configurable:true,value:${JSON.stringify(name)}},length:{configurable:true,value:${length}}});`;
}

function hostAliasDefinitions(module) {
  const definitions = new Map();
  for (const match of module.matchAll(
    /import\{([^}]*)\}from["'](?:\.\.\/)*(?:\.\/)?host\.js["'];?/g,
  )) {
    for (const entry of match[1].split(",")) {
      const [imported, local = imported] = entry.trim().split(/\s+as\s+/);
      if (local !== imported) definitions.set(local, imported);
    }
  }
  return [...definitions]
    .map(([local, imported]) => `const ${local}=${imported};`)
    .join("");
}

function prepareModule(module, publicOnly, coreSpecifier) {
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
    const kept = entries.filter(({ publicName }) => !publicOnly || !internalTestExports.has(publicName));
    const statement = `export{${kept.map(({ local, publicName }) =>
      local === publicName ? local : `${local} as ${publicName}`).join(",")}};`;
    return `${definitions.join("")}${statement}export*from${JSON.stringify(coreSpecifier)};`;
  });
}

function selectProjectFlags(directory) {
  const flags = new Map([
    ["CSS_MODULE_DEV", false],
    ["CSS_MODULE_GLOBAL", false],
    ["CSS_VARS_BROWSER", true],
    ["CSS_VARS_TEST", false],
    ["CSS_VARS_DEV", false],
    ["CSS_VARS_FEATURE_SUSPENSE", true],
    ["RUNTIME_DOM_DEV", false],
    ["RUNTIME_DOM_COMPAT", false],
    ["RUNTIME_DOM_SSR", true],
    ["ATTR_COMPAT", false],
    ["STYLE_DEV", false],
    ["PROP_COMPAT", false],
    ["PROP_DEV", false],
    ["EVENT_DEV", false],
    ["CUSTOM_ELEMENT_DEV", false],
    ["CUSTOM_ELEMENT_FEATURE_PROD_DEVTOOLS", false],
    ["V_ON_COMPAT", false],
    ["V_ON_DEV", false],
    ["V_MODEL_COMPAT", false],
    ["TRANSITION_GROUP_DEV", false],
    ["TRANSITION_GROUP_COMPAT", false],
    ["TRANSITION_DEV", false],
    ["TRANSITION_COMPAT", false],
  ]);
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) selectProjectFlags(path);
    else if (entry.name.endsWith(".lil")) {
      writeFileSync(path, readFileSync(path, "utf8").replace(
        /^bool ([A-Z][A-Z0-9_]*) = (?:true|false);$/gmu,
        (statement, name) => flags.has(name)
          ? `bool ${name} = ${flags.get(name)};`
          : statement,
      ));
    }
  }
}

try {
  if (!projectVariant) {
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

    const browserRoot = resolve(temporary, "browser");
  const browserSourceDirectory = resolve(browserRoot, "src/runtime-dom");
  const browserDependencies = resolve(browserRoot, "packages/vuelil");
  mkdirSync(resolve(browserRoot, "src"), { recursive: true });
  mkdirSync(browserDependencies, { recursive: true });
  cpSync(resolve(projectRoot, "src/runtime-dom"), browserSourceDirectory, { recursive: true });
  copyFileSync(
    resolve(projectRoot, "tests/vue-runtime-core-browser.candidate.mjs"),
    resolve(browserDependencies, "runtime-core.js"),
  );
  copyFileSync(
    resolve(projectRoot, "packages/vuelil/shared.js"),
    resolve(browserDependencies, "shared.js"),
  );
  const browserFlags = new Map([
    ["CSS_MODULE_GLOBAL", true],
    ["CSS_VARS_BROWSER", true],
    ["CSS_VARS_TEST", false],
    ["RUNTIME_DOM_SSR", false],
  ]);
  const selectBrowserFlags = directory => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = resolve(directory, entry.name);
      if (entry.isDirectory()) selectBrowserFlags(path);
      else if (entry.name.endsWith(".lil")) {
        writeFileSync(path, readFileSync(path, "utf8").replace(
          /^bool ([A-Z][A-Z0-9_]*) = (?:true|false);$/gmu,
          (statement, name) => browserFlags.has(name)
            ? `bool ${name} = ${browserFlags.get(name)};`
            : statement,
        ));
      }
    }
  };
  selectBrowserFlags(browserSourceDirectory);
  const browserResult = spawnSync(
    compilerPath(),
    [
      resolve(browserSourceDirectory, "index.lil"),
      "--target", "js-module",
      "--mode", "development",
      "--config", resolve(repositoryRoot, "tests/config/no-optimization-no-peephole.toml"),
      "-o", browserCompiled,
    ],
    { cwd: projectRoot, encoding: "utf8", env: process.env },
  );
    if (browserResult.status !== 0) {
      throw new Error(`${browserResult.stdout ?? ""}${browserResult.stderr ?? ""}`);
    }
  }

  const projectRootBuild = resolve(temporary, "project");
  const projectSourceDirectory = resolve(projectRootBuild, "src/runtime-dom");
  const projectDependencies = resolve(projectRootBuild, "packages/vuelil");
  mkdirSync(resolve(projectRootBuild, "src"), { recursive: true });
  mkdirSync(projectDependencies, { recursive: true });
  cpSync(resolve(projectRoot, "src/runtime-dom"), projectSourceDirectory, { recursive: true });
  copyFileSync(
    projectRuntimeCoreInput,
    resolve(projectDependencies, "runtime-core.js"),
  );
  copyFileSync(
    projectSharedInput,
    resolve(projectDependencies, "shared.js"),
  );
  selectProjectFlags(projectSourceDirectory);
  const projectStyle = resolve(projectSourceDirectory, "modules/style.lil");
  const projectStyleSource = readFileSync(projectStyle, "utf8");
  const nestedPreviousStyle = `    if (!JS.isNullish(previous) && previous.truthy()) {
      if (!styleIsString(previous)) {
        for (string key in previous) {
          if (JS.isNullish(hostRead(next, key))) setStyle(style, key, "");
        }
      } else {
        string[] oldStyles = hostString(previous).split(";");
        for (int index = 0; index < oldStyles.length; index++) {
          string oldStyle = oldStyles[index];
          int colon = oldStyle.indexOf(":");
          string key = oldStyle.slice(0, colon).trim();
          if (JS.isNullish(hostRead(next, key))) setStyle(style, key, "");
        }
      }
    }`;
  const flattenedPreviousStyle = `    if (!JS.isNullish(previous) && previous.truthy() && !styleIsString(previous)) {
      for (string key in previous) {
        if (JS.isNullish(hostRead(next, key))) setStyle(style, key, "");
      }
    }
    if (!JS.isNullish(previous) && previous.truthy() && styleIsString(previous)) {
      string[] oldStyles = hostString(previous).split(";");
      for (int index = 0; index < oldStyles.length; index++) {
        string oldStyle = oldStyles[index];
        int colon = oldStyle.indexOf(":");
        string key = oldStyle.slice(0, colon).trim();
        if (JS.isNullish(hostRead(next, key))) setStyle(style, key, "");
      }
    }`;
  if (!projectStyleSource.includes(nestedPreviousStyle)) {
    throw new Error("failed to select the optimizer-safe production style shape");
  }
  writeFileSync(
    projectStyle,
    projectStyleSource.replace(nestedPreviousStyle, flattenedPreviousStyle),
  );
  const projectIndex = resolve(projectSourceDirectory, "index.lil");
  const retainedOwners = new Set(["./nodeOps", "./patchProp", "./host.js"]);
  const slimmedIndex = readFileSync(projectIndex, "utf8")
    .replace(
      /import\s+(extern\s+)?\{([\s\S]*?)\}\s*from\s*"([^"]+)"\s*;/gu,
      (statement, _extern, _bindings, specifier) =>
        specifier.startsWith("../../packages/vuelil/") || retainedOwners.has(specifier)
          ? statement
          : "",
    )
    .replace(
      /export\s*\{[\s\S]*?\}\s*;\s*$/u,
      `export { ${projectExports.filter(name => name !== "createApp").join(", ")} };\n`,
    )
    .replace(
      /bool ssrDirectiveInitialized = false;\s*export void initDirectivesForSSR\(\) \{[\s\S]*?\n\}/u,
      "export void initDirectivesForSSR() {}",
    );
  writeFileSync(projectIndex, slimmedIndex);
  const projectEntry = resolve(projectSourceDirectory, "project-entry.lil");
  writeFileSync(
    projectEntry,
    `import { ${projectExports.join(", ")} } from "./index";\n` +
      `export { ${projectExports.join(", ")} };\n`,
  );
  const projectResult = spawnSync(
    compilerPath(),
    [
      projectEntry,
      "--target", "js-module",
      "--mode", "production",
      "--config", resolve(projectRoot, "config/open-world.toml"),
      "--jobs", "1",
      "--codec-jobs", "1",
      "-o", projectCompiled,
    ],
    { cwd: projectRoot, encoding: "utf8", env: process.env },
  );
  if (projectResult.status !== 0) {
    throw new Error(`${projectResult.stdout ?? ""}${projectResult.stderr ?? ""}`);
  }

  if (!projectVariant) {
    const productionPackageRoot = resolve(temporary, "production-package");
  const productionPackageSource = resolve(productionPackageRoot, "src/runtime-dom");
  const productionPackageDependencies = resolve(
    productionPackageRoot,
    "packages/vuelil",
  );
  mkdirSync(resolve(productionPackageRoot, "src"), { recursive: true });
  mkdirSync(productionPackageDependencies, { recursive: true });
  cpSync(
    resolve(projectRoot, "src/runtime-dom"),
    productionPackageSource,
    { recursive: true },
  );
  copyFileSync(
    resolve(productionDirectory, "runtime-core.js"),
    resolve(productionPackageDependencies, "runtime-core.js"),
  );
  copyFileSync(
    resolve(productionDirectory, "shared.js"),
    resolve(productionPackageDependencies, "shared.js"),
  );
  selectProjectFlags(productionPackageSource);
  const productionPackageExports = exportedNames(
    readFileSync(compiled, "utf8"),
    true,
  );
  const productionPackageEntry = resolve(
    productionPackageSource,
    "production-package-entry.lil",
  );
  writeFileSync(
    productionPackageEntry,
    `import { ${productionPackageExports.join(", ")} } from "./index";\n` +
      `export { ${productionPackageExports.join(", ")} };\n`,
  );
  const productionPackageResult = spawnSync(
    compilerPath(),
    [
      productionPackageEntry,
      "--target", "js-module",
      "--mode", "production",
      "--config", resolve(projectRoot, "config/open-world.toml"),
      "--jobs", "1",
      "--codec-jobs", "1",
      "-o", productionPackageCompiled,
    ],
    { cwd: projectRoot, encoding: "utf8", env: process.env },
  );
    if (productionPackageResult.status !== 0) {
      throw new Error(
        `${productionPackageResult.stdout ?? ""}${productionPackageResult.stderr ?? ""}`,
      );
    }
  }

  const hostModule = readFileSync(host, "utf8").replaceAll("export function ", "function ");
  if (!projectVariant) {
    const compiledSource = readFileSync(compiled, "utf8");
  const hostAliases = hostAliasDefinitions(compiledSource);
  let compiledModule = compiledSource
    .replaceAll('"../../packages/vuelil/runtime-core.js"', '"./runtime-core.js"')
    .replaceAll('"../../packages/vuelil/shared.js"', '"./shared.js"')
    .replace(/import\{[^;]*\}from["'](?:\.\.\/)*(?:\.\/)?host\.js["'];?/g, "");
  if (/from["'](?:\.\.\/)*(?:\.\/)?host\.js["']/.test(compiledModule)) {
    throw new Error("failed to inline the runtime-dom host adapter");
  }

  const banner = "// Generated from src/runtime-dom/index.lil and its measured DOM host adapter.\n";
  mkdirSync(resolve(projectRoot, "packages/vuelil"), { recursive: true });
  writeFileSync(output, `${banner}${hostModule}\n${hostAliases}\n${prepareModule(compiledModule, true, "./runtime-core.js")}\n`);

  const testModule = compiledModule
    .replaceAll('"./runtime-core.js"', '"./runtime-core-upstream.candidate.mjs"')
    .replaceAll('"./shared.js"', '"../packages/vuelil/shared.js"');
  writeFileSync(
    testOutput,
    `${banner}${hostModule}\n${hostAliases}\n${prepareModule(testModule, false, "./runtime-core-upstream.candidate.mjs")}\n`,
  );
  const browserCompiledModule = readFileSync(browserCompiled, "utf8")
    .replaceAll('"../../packages/vuelil/runtime-core.js"', '"./vue-runtime-core-browser.candidate.mjs"')
    .replaceAll('"../../packages/vuelil/shared.js"', '"../packages/vuelil/shared.js"')
    .replace(/import\{[^;]*\}from["'](?:\.\.\/)*(?:\.\/)?host\.js["'];?/g, "");
  const browserHostAliases = hostAliasDefinitions(readFileSync(browserCompiled, "utf8"));
    writeFileSync(
      browserOutput,
      `${banner}${hostModule}\n${browserHostAliases}\n${prepareModule(browserCompiledModule, true, "./vue-runtime-core-browser.candidate.mjs")}\n`,
    );
  }
  const banner = "// Generated from src/runtime-dom/index.lil and its measured DOM host adapter.\n";
  const projectCompiledSource = readFileSync(projectCompiled, "utf8");
  const projectHostAliases = hostAliasDefinitions(projectCompiledSource);
  const projectModule = projectCompiledSource
    .replace(
      /["']\.\.\/\.\.\/packages\/vuelil\/runtime-core\.js["']/gu,
      JSON.stringify(projectVariant ? "./runtime-core.js" : "./runtime-core.project.generated.js"),
    )
    .replace(
      /["']\.\.\/\.\.\/packages\/vuelil\/shared\.js["']/gu,
      JSON.stringify(projectVariant ? "./shared.js" : "./shared.project.generated.js"),
    )
    .replace(/import\{[^;]*\}from["'](?:\.\.\/)*(?:\.\/)?host\.js["'];?/g, "");
  if (/from["'](?:\.\.\/)*(?:\.\/)?host\.js["']/.test(projectModule)) {
    throw new Error("failed to inline the project runtime-dom host adapter");
  }
  writeFileSync(
    projectOutput,
    `${banner}${hostModule}\n${projectHostAliases}\n${prepareModule(
      projectModule,
      true,
      projectVariant ? "./runtime-core.js" : "./runtime-core.project.generated.js",
    )}\n`,
  );
  if (projectVueOutput) {
    writeFileSync(
      projectVueOutput,
      'export * from "./runtime-dom.js";\nexport function compile() {}\n',
    );
  }
  if (!projectVariant) {
    const productionPackageCompiledSource = readFileSync(
      productionPackageCompiled,
      "utf8",
    );
    const productionPackageHostAliases = hostAliasDefinitions(
      productionPackageCompiledSource,
    );
    const productionPackageModule = productionPackageCompiledSource
      .replace(
        /["']\.\.\/\.\.\/packages\/vuelil\/runtime-core\.js["']/gu,
        '"./runtime-core.js"',
      )
      .replace(
        /["']\.\.\/\.\.\/packages\/vuelil\/shared\.js["']/gu,
        '"./shared.js"',
      )
      .replace(/import\{[^;]*\}from["'](?:\.\.\/)*(?:\.\/)?host\.js["'];?/g, "");
    if (/from["'](?:\.\.\/)*(?:\.\/)?host\.js["']/.test(productionPackageModule)) {
      throw new Error("failed to inline the production runtime-dom host adapter");
    }
    mkdirSync(productionDirectory, { recursive: true });
    writeFileSync(
      productionOutput,
      `${banner}${hostModule}\n${productionPackageHostAliases}\n${prepareModule(
        productionPackageModule,
        true,
        "./runtime-core.js",
      )}\n`,
    );

    const runtime = await import(`../packages/vuelil/runtime-dom.js?build=${Date.now()}`);
    console.log(JSON.stringify({
      output,
      browserOutput,
      projectOutput,
      productionOutput,
      exports: Object.keys(runtime).sort(),
      bytes: readFileSync(output).byteLength,
    }));
  } else {
    console.log(JSON.stringify({ projectOutput, projectVueOutput, exports: projectExports }));
  }
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
