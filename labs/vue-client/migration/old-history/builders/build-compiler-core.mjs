import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { compilerPath, projectRoot } from "../tooling/compiler-path.mjs";

const source = resolve(projectRoot, "src/compiler-core/index.lil");
const sourceDirectory = resolve(projectRoot, "src/compiler-core");
const sharedSourceDirectory = resolve(projectRoot, "src/shared");
const astSource = resolve(projectRoot, "src/compiler-core/ast.lil");
const babelUtilsSource = resolve(projectRoot, "src/compiler-core/babelUtils.lil");
const codegenSource = resolve(projectRoot, "src/compiler-core/codegen.lil");
const compatConfigSource = resolve(
  projectRoot,
  "src/compiler-core/compat/compatConfig.lil",
);
const compileSource = resolve(projectRoot, "src/compiler-core/compile.lil");
const errorsSource = resolve(projectRoot, "src/compiler-core/errors.lil");
const parserSource = resolve(projectRoot, "src/compiler-core/parser.lil");
const runtimeHelpersSource = resolve(
  projectRoot,
  "src/compiler-core/runtimeHelpers.lil",
);
const transformSource = resolve(projectRoot, "src/compiler-core/transform.lil");
const utilsSource = resolve(projectRoot, "src/compiler-core/utils.lil");
const cacheStaticSource = resolve(
  projectRoot,
  "src/compiler-core/transforms/cacheStatic.lil",
);
const noopDirectiveTransformSource = resolve(
  projectRoot,
  "src/compiler-core/transforms/noopDirectiveTransform.lil",
);
const transformElementSource = resolve(
  projectRoot,
  "src/compiler-core/transforms/transformElement.lil",
);
const transformTextSource = resolve(
  projectRoot,
  "src/compiler-core/transforms/transformText.lil",
);
const transformExpressionSource = resolve(
  projectRoot,
  "src/compiler-core/transforms/transformExpression.lil",
);
const transformSlotOutletSource = resolve(
  projectRoot,
  "src/compiler-core/transforms/transformSlotOutlet.lil",
);
const transformVBindShorthandSource = resolve(
  projectRoot,
  "src/compiler-core/transforms/transformVBindShorthand.lil",
);
const vBindSource = resolve(projectRoot, "src/compiler-core/transforms/vBind.lil");
const vForSource = resolve(projectRoot, "src/compiler-core/transforms/vFor.lil");
const vIfSource = resolve(projectRoot, "src/compiler-core/transforms/vIf.lil");
const vMemoSource = resolve(projectRoot, "src/compiler-core/transforms/vMemo.lil");
const vModelSource = resolve(projectRoot, "src/compiler-core/transforms/vModel.lil");
const vOnSource = resolve(projectRoot, "src/compiler-core/transforms/vOn.lil");
const vOnceSource = resolve(projectRoot, "src/compiler-core/transforms/vOnce.lil");
const vSlotSource = resolve(projectRoot, "src/compiler-core/transforms/vSlot.lil");
const host = resolve(projectRoot, "src/compiler-core/host.js");
const output = resolve(projectRoot, "packages/vuelil/compiler-core.js");
const productionOutput = resolve(
  projectRoot,
  "packages/vuelil/production/compiler-core.js",
);
const productionExports = JSON.parse(
  readFileSync(resolve(projectRoot, "compatibility/compiler-core.json"), "utf8"),
).runtimeExports;
const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-compiler-core-"));
const compiled = resolve(temporary, "index.js");
const productionCompiled = resolve(temporary, "production.js");
const temporarySourceDirectory = resolve(temporary, "compiler-core");
const temporarySource = resolve(temporarySourceDirectory, "index.lil");
const sourceText = [
  source,
  astSource,
  babelUtilsSource,
  codegenSource,
  compatConfigSource,
  compileSource,
  errorsSource,
  parserSource,
  runtimeHelpersSource,
  transformSource,
  utilsSource,
  cacheStaticSource,
  noopDirectiveTransformSource,
  transformElementSource,
  transformExpressionSource,
  transformSlotOutletSource,
  transformTextSource,
  transformVBindShorthandSource,
  vBindSource,
  vForSource,
  vIfSource,
  vMemoSource,
  vModelSource,
  vOnSource,
  vOnceSource,
  vSlotSource,
]
  .map(path => readFileSync(path, "utf8"))
  .join("\n");
const publicFunctions = new Set(
  [...sourceText.matchAll(/export\s+(?:JsValue|bool|int|string|void)\s+(\w+)\s*\(/g)].map(
    match => match[1],
  ),
);
publicFunctions.add("generateCodeFrame");
const correctedLengths = new Map([
  ["advancePositionWithClone", 2],
  ["advancePositionWithMutation", 2],
  ["assert", 2],
  ["baseCompile", 1],
  ["buildProps", 2],
  ["buildSlots", 2],
  ["createArrayExpression", 1],
  ["createCallExpression", 1],
  ["createCompilerError", 4],
  ["createCompoundExpression", 1],
  ["createForLoopParams", 1],
  ["createFunctionExpression", 1],
  ["createObjectExpression", 1],
  ["extractIdentifiers", 1],
  ["generate", 1],
  ["generateCodeFrame", 1],
  ["getBaseTransformPreset", 1],
]);
const correctedNames = new Map([["isText", "isText$1"]]);
const internalExports = new Set(["Array", "JSON", "Object", "String"]);

function compile(input, target, production = false) {
  const args = [input, "--target", "js-module"];
  if (production) {
    args.push(
      "--mode", "production",
      "--config", resolve(projectRoot, "config/open-world.toml"),
      "--jobs", "1",
      "--codec-jobs", "1",
    );
  } else {
    args.push("--mode", "development");
  }
  args.push("-o", target);
  const result = spawnSync(compilerPath(), args, {
    cwd: projectRoot,
    encoding: "utf8",
    env: process.env,
  });
  if (result.status !== 0) {
    throw new Error(`${result.stdout ?? ""}${result.stderr ?? ""}`);
  }
}

function prepareModule(compiledPath, sharedSpecifier) {
  const compiledModule = readFileSync(compiledPath, "utf8")
    .replace(/from["']\.\/shared\.js["']/gu, `from${JSON.stringify(sharedSpecifier)}`)
    .replace(
      /import\s*\{[^}]*\}\s*from\s*["']\.\/host\.js["'];?\s*/u,
      "",
    );
  if (/from\s*["']\.\/host\.js["']/u.test(compiledModule)) {
    throw new Error("failed to inline compiler-core host adapter");
  }
  return compiledModule.replace(/export\{([^}]*)\}\s*$/, (statement, exports) => {
    const definitions = exports
      .split(",")
      .map(entry => {
        const [local, publicName = local] = entry.trim().split(/\s+as\s+/);
        if (!publicFunctions.has(publicName)) return "";
        const length = correctedLengths.get(publicName);
        const reflectedName = correctedNames.get(publicName) ?? publicName;
        const descriptors = [`name:{configurable:true,value:${JSON.stringify(reflectedName)}}`];
        if (length !== undefined) {
          descriptors.push(`length:{configurable:true,value:${length}}`);
        }
        return `Object.defineProperties(${local},{${descriptors.join(",")}});`;
      })
      .join("");
    const kept = exports
      .split(",")
      .filter(entry => !internalExports.has(entry.trim().split(/\s+as\s+/).at(-1)));
    return `${definitions}export{${kept.join(",")}}`;
  });
}

try {
  cpSync(sourceDirectory, temporarySourceDirectory, { recursive: true });
  cpSync(sharedSourceDirectory, resolve(temporary, "shared"), { recursive: true });
  const canonicalBarrel = readFileSync(temporarySource, "utf8");
  // Emit the cross-package live binding without linking duplicate host globals.
  const temporaryBarrel = canonicalBarrel.replace(
    'import { generateCodeFrame } from "../shared/codeframe";',
    'import extern { generateCodeFrame } from "./shared.js";\nextern JsValue generateCodeFrame;',
  );
  if (temporaryBarrel === canonicalBarrel) {
    throw new Error("failed to install compiler-core shared export bridge");
  }
  writeFileSync(temporarySource, temporaryBarrel);
  writeFileSync(
    resolve(temporarySourceDirectory, "shared.js"),
    "export const generateCodeFrame = undefined;\n",
  );
  copyFileSync(host, resolve(temporary, "host.js"));
  compile(temporarySource, compiled);

  const validateExpression = resolve(temporarySourceDirectory, "validateExpression.lil");
  const validationSource = readFileSync(validateExpression, "utf8")
    .replace("export bool COMPILER_BROWSER = false;", "export bool COMPILER_BROWSER = true;")
    .replace("export bool COMPILER_DEV = true;", "export bool COMPILER_DEV = false;");
  writeFileSync(validateExpression, validationSource);
  const compilerUtils = resolve(temporarySourceDirectory, "utils.lil");
  const compilerUtilsSource = readFileSync(compilerUtils, "utf8")
    .replace(
      "export JsValue isMemberExpression = isMemberExpressionNode;",
      "export JsValue isMemberExpression = isMemberExpressionBrowser;",
    )
    .replace(
      "export JsValue isFnExpression = isFnExpressionNode;",
      "export JsValue isFnExpression = isFnExpressionBrowser;",
    );
  writeFileSync(compilerUtils, compilerUtilsSource);
  const transformExpression = resolve(
    temporarySourceDirectory,
    "transforms/transformExpression.lil",
  );
  const transformExpressionSource = readFileSync(transformExpression, "utf8");
  const processStart = transformExpressionSource.indexOf(
    "export JsValue processExpression(",
  );
  const processBodyStart = transformExpressionSource.indexOf(") {", processStart) + 3;
  const processBodyEnd = transformExpressionSource.indexOf(
    "\n}\n\nexport JsValue transformExpression",
    processBodyStart,
  );
  if (processStart < 0 || processBodyStart < 3 || processBodyEnd < 0) {
    throw new Error("failed to select browser production expression processing");
  }
  writeFileSync(
    transformExpression,
    `${transformExpressionSource.slice(0, processBodyStart)}\n  return node;` +
      transformExpressionSource.slice(processBodyEnd),
  );
  const compilerParser = resolve(temporarySourceDirectory, "parser.lil");
  const compilerParserSource = readFileSync(compilerParser, "utf8")
    .replace(
      'import { createRoot, createSimpleExpression } from "./ast";',
      'import { createRoot, createSimpleExpression } from "./ast";\n' +
        'import { COMPILER_BROWSER } from "./validateExpression";',
    )
    .replace(
      'if (!isStatic && currentOptions["prefixIdentifiers"].truthy() && parseMode != 3 && content.trim() != "") {',
      'if (!COMPILER_BROWSER && !isStatic && currentOptions["prefixIdentifiers"].truthy() && parseMode != 3 && content.trim() != "") {',
    );
  writeFileSync(compilerParser, compilerParserSource);
  const productionEntry = resolve(temporarySourceDirectory, "production.lil");
  writeFileSync(
    productionEntry,
    [
      `import { ${productionExports.join(", ")} } from "./index";`,
      `export { ${productionExports.join(", ")} };`,
      "",
    ].join("\n"),
  );
  compile(productionEntry, productionCompiled, true);

  mkdirSync(resolve(projectRoot, "packages/vuelil"), { recursive: true });
  const hostModule = readFileSync(host, "utf8").replaceAll("export function ", "function ");
  const productionHostModule = hostModule.replace(
    'import { parse, parseExpression } from "@babel/parser";\n',
    "",
  );
  const namedModule = prepareModule(compiled, "./shared.js");
  const productionModule = prepareModule(productionCompiled, "./shared.js");
  writeFileSync(
    output,
    `// Generated from compiler-core LilScript sources. Template parsing is LilScript-owned.\n${hostModule}\n${namedModule}`,
  );
  mkdirSync(resolve(productionOutput, ".."), { recursive: true });
  writeFileSync(
    productionOutput,
    `// Optimized browser production compiler-core.\n${productionHostModule}\n${productionModule}`,
  );
  console.log(JSON.stringify({
    output,
    productionOutput,
    exports: await exportedNames(output),
  }));
} finally {
  rmSync(temporary, { force: true, recursive: true });
}

async function exportedNames(path) {
  return Object.keys(await import(`${path}?build=${Date.now()}`)).sort();
}
