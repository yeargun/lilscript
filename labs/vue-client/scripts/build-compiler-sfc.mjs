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
const warnCandidate = resolve(projectRoot, "tests/compiler-sfc-warn.candidate.mjs");
const reflectedFunctions = new Map([
  ["compileStyle", ["compileStyle", 1]],
  ["compileStyleAsync", ["compileStyleAsync", 1]],
  ["compileScript", ["compileScript", 2]],
  ["compileTemplate", ["compileTemplate", 1]],
  ["extractRuntimeEmits", ["extractRuntimeEmits", 1]],
  ["extractRuntimeProps", ["extractRuntimeProps", 1]],
  ["inferRuntimeType", ["inferRuntimeType", 2]],
  ["invalidateTypeCache", ["invalidateTypeCache", 1]],
  ["parse", ["parse", 1]],
  ["registerTS", ["registerTS", 1]],
  ["resolveTypeElements", ["resolveTypeElements", 4]],
  ["rewriteDefault", ["rewriteDefault", 2]],
  ["rewriteDefaultAST", ["rewriteDefaultAST", 3]],
  ["shouldTransformRef", ["shouldTransformRef", 0]],
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

function assemble(compiledPath) {
  let module = readFileSync(compiledPath, "utf8").replace(
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
  const hostAliases = "const assetHostAssign=hostAssign,assetHostKeys=hostKeys,templateAstHostAssign=hostAssign,parseHostAssign=hostAssign,contextCreateSet=hostCreateSet,usageCreateSet=hostCreateSet,preprocessAssign=hostAssign,preprocessJsonParse=hostJsonParse,preprocessRequire=hostRequire,scopedApply=hostApply,scopedCreateNullObject=hostCreateNullObject,scopedKeys=hostKeys,compileStyleAssign=hostAssign,compileStyleCreateError=hostCreateError,compileStyleCreateSet=hostCreateSet,templateAssign=hostAssign,templateCreateError=hostCreateError,templateRequire=hostRequire,compileScriptArrayFrom=hostArrayFrom,compileScriptHostAssign=hostAssign,compileScriptCreateError=hostCreateError,compileScriptCreateObject=hostCreateObject,compileScriptCreateSet=hostCreateSet,compileScriptJsonStringify=hostJsonStringify,compileScriptKeys=hostKeys,destructureCreateNullObject=hostCreateNullObject,destructureCreateObject=hostCreateObject,emitsArrayFrom=hostArrayFrom,emitsCreateSet=hostCreateSet,emitsJsonStringify=hostJsonStringify,emitsKeys=hostKeys,modelJsonStringify=hostJsonStringify,modelKeys=hostKeys,indexAssign=hostAssign;";
  return reflectFunctions(
    `// Generated from compiler-sfc LilScript owner modules.\n${hostModule}\n${hostAliases}\n${module}`,
  );
}

const temporary = mkdtempSync(resolve(tmpdir(), "vuelil-compiler-sfc-"));
const graph = resolve(temporary, "src/compiler-sfc");
const entry = resolve(graph, "test-entry.lil");
const compiled = resolve(temporary, "candidate.mjs");
const testGraph = resolve(temporary, "src/compiler-sfc-test");
const compiledTest = resolve(temporary, "test-candidate.mjs");
const compiledWarn = resolve(temporary, "warn-candidate.mjs");

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
  const compileScriptPath = resolve(graph, "compileScript.lil");
  writeFileSync(
    compileScriptPath,
    readFileSync(compileScriptPath, "utf8").replace(
      "bool compileScriptTestBuild = false;",
      "bool compileScriptTestBuild = true;",
    ),
  );
  const resolveTypePath = resolve(graph, "script/resolveType.lil");
  writeFileSync(
    resolveTypePath,
    readFileSync(resolveTypePath, "utf8").replace(
      "bool typeResolveTestBuild = false;",
      "bool typeResolveTestBuild = true;",
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
      'import { MagicString, babelParse, errorMessages, extractIdentifiers, generateCodeFrame, isInDestructureAssignment, isStaticProperty, shouldTransformRef, version, walk, walkIdentifiers } from "./index";',
      'import { compileStyle, compileStyleAsync, doCompileStyle } from "./compileStyle";',
      'import { compileScript, mergeSourceMaps } from "./compileScript";',
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
      'import { DEFINE_EXPOSE, processDefineExpose } from "./script/defineExpose";',
      'import { DEFINE_SLOTS, processDefineSlots } from "./script/defineSlots";',
      'import { DEFINE_EMITS, extractRuntimeEmits, genRuntimeEmits, processDefineEmits } from "./script/defineEmits";',
      'import { DEFINE_MODEL, genModelProps, processDefineModel } from "./script/defineModel";',
      'import { DEFINE_OPTIONS, processDefineOptions } from "./script/defineOptions";',
      'import { DEFINE_PROPS, WITH_DEFAULTS, extractRuntimeProps, genRuntimeProps, processDefineProps } from "./script/defineProps";',
      'import { processPropsDestructure, transformDestructuredProps } from "./script/definePropsDestructure";',
      'import { TypeScope, fileToScope, inferRuntimeType, invalidateTypeCache, recordImports, registerTS, resolveTypeElements, resolveUnionType } from "./script/resolveType";',
      'import { isImportUsed, resolveTemplateVModelIdentifiers, templateAnalysisCache } from "./script/importUsageCheck";',
      'import { normalScriptDefaultVar, processNormalScript } from "./script/normalScript";',
      'import { processAwait } from "./script/topLevelAwait";',
      'import { UNKNOWN_TYPE, concatStrings, createGetCanonicalFileName, getEscapedPropName, getId, getImportedName, getStringLiteralKey, isCallOf, isJS, isLiteralNode, isTS, joinPaths, normalizePath, propNameEscapeSymbolsRE, resolveObjectKey, toRuntimeTypeString } from "./script/utils";',
      "export { MagicString, babelParse, errorMessages, extractIdentifiers, generateCodeFrame, isInDestructureAssignment, isStaticProperty, shouldTransformRef, version, walk, walkIdentifiers, createCache, compileScript, mergeSourceMaps, compileStyle, compileStyleAsync, doCompileStyle, compileTemplate, doCompileTemplate, DEFAULT_FILENAME, hmrShouldReload, parse, parseCache, rewriteDefault, rewriteDefaultAST, hasDefaultExport, ScriptCompileContext, resolveParserPlugins, warn, warnOnce, trimPlugin, CSS_VARS_HELPER, cssVarsPlugin, genCssVarsCode, genCssVarsFromList, genNormalScriptCssVarsCode, parseCssVars, processors, scopedPlugin, isDataUrl, isExternalUrl, isRelativeUrl, normalizeDecodedImportPath, parseUrl, createAssetUrlTransformWithOptions, defaultAssetUrlOptions, normalizeOptions, transformAssetUrl, createSrcsetTransformWithOptions, transformSrcset, resolveTemplateAST, analyzeScriptBindings, getObjectOrArrayExpressionKeys, DEFINE_EMITS, extractRuntimeEmits, genRuntimeEmits, processDefineEmits, DEFINE_EXPOSE, processDefineExpose, DEFINE_MODEL, genModelProps, processDefineModel, DEFINE_OPTIONS, processDefineOptions, DEFINE_PROPS, WITH_DEFAULTS, extractRuntimeProps, genRuntimeProps, processDefineProps, processPropsDestructure, transformDestructuredProps, DEFINE_SLOTS, processDefineSlots, TypeScope, fileToScope, inferRuntimeType, invalidateTypeCache, recordImports, registerTS, resolveTypeElements, resolveUnionType, isImportUsed, resolveTemplateVModelIdentifiers, templateAnalysisCache, normalScriptDefaultVar, processNormalScript, processAwait, UNKNOWN_TYPE, concatStrings, createGetCanonicalFileName, getEscapedPropName, getId, getImportedName, getStringLiteralKey, isCallOf, isJS, isLiteralNode, isTS, joinPaths, normalizePath, propNameEscapeSymbolsRE, resolveObjectKey, toRuntimeTypeString };",
      "",
    ].join("\n"),
  );
  runCompiler(entry, compiled);
  runCompiler(warnPath, compiledWarn);
  cpSync(graph, testGraph, { recursive: true });
  copyFileSync(
    compiledWarn,
    resolve(testGraph, "compiler-sfc-warn.candidate.mjs"),
  );
  writeFileSync(
    resolve(testGraph, "warn.lil"),
    [
      'import extern { warn, warnOnce } from "./compiler-sfc-warn.candidate.mjs";',
      "extern void warn(string msg);",
      "extern void warnOnce(string msg);",
      "export { warn, warnOnce };",
      "",
    ].join("\n"),
  );
  runCompiler(resolve(testGraph, "test-entry.lil"), compiledTest);
  const module = assemble(compiled);
  const testModule = assemble(compiledTest);
  const warnEdge = /from\s*["']\.\/compiler-sfc-warn\.candidate\.mjs["']/u;
  if (warnEdge.test(module)) {
    throw new Error("production compiler-sfc unexpectedly imports test warn module");
  }
  if (!warnEdge.test(testModule)) {
    throw new Error("compiler-sfc test candidate did not externalize warn module");
  }
  mkdirSync(resolve(projectRoot, "artifacts"), { recursive: true });
  mkdirSync(resolve(projectRoot, "packages/vuelil"), { recursive: true });
  writeFileSync(artifact, module);
  writeFileSync(
    facade,
    [
      'export { MagicString, babelParse, compileScript, compileStyle, compileStyleAsync, compileTemplate, errorMessages, extractIdentifiers, extractRuntimeEmits, extractRuntimeProps, generateCodeFrame, inferRuntimeType, invalidateTypeCache, isInDestructureAssignment, isStaticProperty, parse, parseCache, registerTS, resolveTypeElements, rewriteDefault, rewriteDefaultAST, shouldTransformRef, version, walk, walkIdentifiers } from "../../artifacts/compiler-sfc.generated.js";',
      "",
    ].join("\n"),
  );
  writeFileSync(candidate, testModule);
  writeFileSync(warnCandidate, readFileSync(compiledWarn, "utf8"));
  console.log(JSON.stringify({ artifact, candidate, facade }));
} finally {
  rmSync(temporary, { force: true, recursive: true });
}
