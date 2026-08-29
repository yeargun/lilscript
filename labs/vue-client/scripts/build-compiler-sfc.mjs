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

const sourceDirectory = resolve(projectRoot, "src/compiler-sfc");
const host = resolve(sourceDirectory, "host.js");
const artifact = resolve(projectRoot, "artifacts/compiler-sfc.generated.js");
const facade = resolve(projectRoot, "packages/vuelil/compiler-sfc.js");
const candidate = resolve(projectRoot, "tests/compiler-sfc-upstream.candidate.mjs");
const reflectedFunctions = new Map([
  ["compileStyle", ["compileStyle", 1]],
  ["compileStyleAsync", ["compileStyleAsync", 1]],
  ["compileTemplate", ["compileTemplate", 1]],
  ["parse", ["parse", 1]],
  ["rewriteDefault", ["rewriteDefault", 2]],
  ["rewriteDefaultAST", ["rewriteDefaultAST", 3]],
]);

function reflectFunctions(module) {
  return module.replace(/export\{([^}]*)\}\s*$/u, (statement, bindings) => {
    const definitions = bindings
      .split(",")
      .map(binding => {
        const [local, publicName = local] = binding.trim().split(/\s+as\s+/u);
        const reflection = reflectedFunctions.get(publicName);
        if (!reflection) return "";
        const [name, length] = reflection;
        return `Object.defineProperties(${local},{name:{configurable:true,value:${JSON.stringify(name)}},length:{configurable:true,value:${length}}});`;
      })
      .join("");
    return `${definitions}${statement}`;
  });
}

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

const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-compiler-sfc-"));
const graph = resolve(temporary, "src/compiler-sfc");
const entry = resolve(graph, "test-entry.lil");
const compiled = resolve(temporary, "candidate.mjs");

try {
  mkdirSync(resolve(temporary, "src"), { recursive: true });
  mkdirSync(resolve(temporary, "packages/vuelil"), { recursive: true });
  cpSync(sourceDirectory, graph, { recursive: true });
  const warnPath = resolve(graph, "warn.lil");
  writeFileSync(
    warnPath,
    readFileSync(warnPath, "utf8").replace(
      "bool testBuild = false;",
      "bool testBuild = true;",
    ),
  );
  copyFileSync(
    resolve(projectRoot, "packages/vuelil/compiler-dom.js"),
    resolve(temporary, "packages/vuelil/compiler-dom.js"),
  );
  copyFileSync(
    resolve(projectRoot, "packages/vuelil/compiler-core.js"),
    resolve(temporary, "packages/vuelil/compiler-core.js"),
  );
  copyFileSync(
    resolve(projectRoot, "packages/vuelil/shared.js"),
    resolve(temporary, "packages/vuelil/shared.js"),
  );
  copyFileSync(
    resolve(projectRoot, "packages/vuelil/compiler-ssr.js"),
    resolve(temporary, "packages/vuelil/compiler-ssr.js"),
  );
  writeFileSync(
    entry,
    [
      'import { createCache } from "./cache";',
      'import { compileStyle, compileStyleAsync, doCompileStyle } from "./compileStyle";',
      'import { compileTemplate, doCompileTemplate } from "./compileTemplate";',
      'import { rewriteDefault, rewriteDefaultAST, hasDefaultExport } from "./rewriteDefault";',
      'import { DEFAULT_FILENAME, hmrShouldReload, parse, parseCache } from "./parse";',
      'import { ScriptCompileContext, resolveParserPlugins } from "./script/context";',
      'import { warn, warnOnce } from "./warn";',
      'import { trimPlugin } from "./style/pluginTrim";',
      'import { CSS_VARS_HELPER, cssVarsPlugin, genCssVarsCode, genCssVarsFromList, genNormalScriptCssVarsCode, parseCssVars } from "./style/cssVars";',
      'import { processors } from "./style/preprocessors";',
      'import { scopedPlugin } from "./style/pluginScoped";',
      'import { isDataUrl, isExternalUrl, isRelativeUrl, normalizeDecodedImportPath, parseUrl } from "./template/templateUtils";',
      'import { createAssetUrlTransformWithOptions, defaultAssetUrlOptions, normalizeOptions, transformAssetUrl } from "./template/transformAssetUrl";',
      'import { createSrcsetTransformWithOptions, transformSrcset } from "./template/transformSrcset";',
      'import { resolveTemplateAST } from "./template/resolveTemplateAST";',
      'import { analyzeScriptBindings, getObjectOrArrayExpressionKeys } from "./script/analyzeScriptBindings";',
      'import { isImportUsed, resolveTemplateVModelIdentifiers, templateAnalysisCache } from "./script/importUsageCheck";',
      'import { normalScriptDefaultVar, processNormalScript } from "./script/normalScript";',
      'import { processAwait } from "./script/topLevelAwait";',
      'import { UNKNOWN_TYPE, concatStrings, createGetCanonicalFileName, getEscapedPropName, getId, getImportedName, getStringLiteralKey, isCallOf, isJS, isLiteralNode, isTS, joinPaths, normalizePath, propNameEscapeSymbolsRE, resolveObjectKey, toRuntimeTypeString } from "./script/utils";',
      "export { createCache, compileStyle, compileStyleAsync, doCompileStyle, compileTemplate, doCompileTemplate, DEFAULT_FILENAME, hmrShouldReload, parse, parseCache, rewriteDefault, rewriteDefaultAST, hasDefaultExport, ScriptCompileContext, resolveParserPlugins, warn, warnOnce, trimPlugin, CSS_VARS_HELPER, cssVarsPlugin, genCssVarsCode, genCssVarsFromList, genNormalScriptCssVarsCode, parseCssVars, processors, scopedPlugin, isDataUrl, isExternalUrl, isRelativeUrl, normalizeDecodedImportPath, parseUrl, createAssetUrlTransformWithOptions, defaultAssetUrlOptions, normalizeOptions, transformAssetUrl, createSrcsetTransformWithOptions, transformSrcset, resolveTemplateAST, analyzeScriptBindings, getObjectOrArrayExpressionKeys, isImportUsed, resolveTemplateVModelIdentifiers, templateAnalysisCache, normalScriptDefaultVar, processNormalScript, processAwait, UNKNOWN_TYPE, concatStrings, createGetCanonicalFileName, getEscapedPropName, getId, getImportedName, getStringLiteralKey, isCallOf, isJS, isLiteralNode, isTS, joinPaths, normalizePath, propNameEscapeSymbolsRE, resolveObjectKey, toRuntimeTypeString };",
      "",
    ].join("\n"),
  );
  runCompiler(entry, compiled);
  let module = readFileSync(compiled, "utf8").replace(
    /["'](?:\.\.\/){2,3}packages\/vuelil\/compiler-dom\.js["']/gu,
    '"../packages/vuelil/compiler-dom.js"',
  );
  module = module.replace(
    /["'](?:\.\.\/){2,3}packages\/vuelil\/shared\.js["']/gu,
    '"../packages/vuelil/shared.js"',
  );
  module = module.replace(
    /["'](?:\.\.\/){2,3}packages\/vuelil\/compiler-core\.js["']/gu,
    '"../packages/vuelil/compiler-core.js"',
  );
  module = module.replace(
    /["'](?:\.\.\/){2,3}packages\/vuelil\/compiler-ssr\.js["']/gu,
    '"../packages/vuelil/compiler-ssr.js"',
  );
  module = module.replace(
    /import\s*\{[^}]*\}\s*from\s*["']\.\/host\.js["'];?/gu,
    "",
  );
  if (/from\s*["']\.\/host\.js["']/u.test(module)) {
    throw new Error("failed to inline compiler-sfc host adapter");
  }
  const hostModule = readFileSync(host, "utf8").replaceAll(
    "export function ",
    "function ",
  );
  const hostAliases = "const assetHostAssign=hostAssign,assetHostKeys=hostKeys,templateAstHostAssign=hostAssign,parseHostAssign=hostAssign,contextCreateSet=hostCreateSet,usageCreateSet=hostCreateSet,preprocessAssign=hostAssign,preprocessJsonParse=hostJsonParse,preprocessRequire=hostRequire,scopedApply=hostApply,scopedCreateNullObject=hostCreateNullObject,scopedKeys=hostKeys,compileStyleAssign=hostAssign,compileStyleCreateError=hostCreateError,compileStyleCreateSet=hostCreateSet,templateAssign=hostAssign,templateCreateError=hostCreateError;";
  module = reflectFunctions(
    `// Generated from compiler-sfc LilScript owner modules.\n${hostModule}\n${hostAliases}\n${module}`,
  );
  mkdirSync(resolve(projectRoot, "artifacts"), { recursive: true });
  mkdirSync(resolve(projectRoot, "packages/vuelil"), { recursive: true });
  writeFileSync(artifact, module);
  writeFileSync(
    facade,
    [
      'export { compileStyle, compileStyleAsync, compileTemplate, parse, parseCache, rewriteDefault, rewriteDefaultAST } from "../../artifacts/compiler-sfc.generated.js";',
      'export { parse as babelParse } from "@babel/parser";',
      'export { default as MagicString } from "magic-string";',
      'export { walk } from "estree-walker";',
      'export { generateCodeFrame, walkIdentifiers, extractIdentifiers, isInDestructureAssignment, isStaticProperty } from "./compiler-core.js";',
      "",
    ].join("\n"),
  );
  writeFileSync(candidate, module);
  console.log(JSON.stringify({ artifact, candidate, facade }));
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
