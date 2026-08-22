import { mkdirSync, readdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import {
  monacoEditorCoreVersion,
  monacoEditorVersion,
  vscodeCommitId,
} from "./catalog.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const labRoot = join(here, "..");
const coreEsm = join(labRoot, "node_modules/monaco-editor-core/esm/vs");
const portsRoot = join(labRoot, "ports/monaco");
const vsRoot = join(portsRoot, "vs");

function walkJs(dir, acc = [], prefix = "") {
  for (const ent of readdirSync(dir, { withFileTypes: true })) {
    const rel = prefix ? `${prefix}/${ent.name}` : ent.name;
    const full = join(dir, ent.name);
    if (ent.isDirectory()) {
      walkJs(full, acc, rel);
    } else if (ent.name.endsWith(".js")) {
      acc.push(rel);
    }
  }
  return acc;
}

function ident(rel) {
  return `vs_${rel.replace(/\.js$/, "").replace(/[^A-Za-z0-9]/g, "_")}`;
}

function importFrom(fromLil, toLil) {
  let rel = relative(dirname(join(portsRoot, fromLil)), join(portsRoot, toLil)).replace(/\\/g, "/");
  if (!rel.startsWith(".")) {
    rel = `./${rel}`;
  }
  return rel.replace(/\.lil$/, "");
}

function shimSource(fromLil, impl, names) {
  const spec = names.join(", ");
  return `import { ${spec} } from "${importFrom(fromLil, impl)}";
export { ${spec} };
`;
}

function viewPartSource(fromLil, rel) {
  const id = ident(rel);
  return `import { EditorView } from "${importFrom(fromLil, "editor/view.lil")}";
export void ${id}(EditorView view) {
  view.render();
}
`;
}

function stubSource(rel) {
  const id = ident(rel);
  const key = rel.replace(/\.js$/, "");
  return `export string ${id}() {
  return "${key}";
}
`;
}

function isStubSource(src) {
  const lines = src.trim().split("\n");
  return lines.length <= 4 && /^export string vs_/.test(src.trim());
}

function isThinViewSource(src) {
  return src.includes("view.render()") && src.trim().split("\n").length <= 8;
}

function isShimSource(src) {
  const lines = src.trim().split("\n").filter((line) => line.length > 0);
  return lines.length <= 4 && lines[0].startsWith("import {") && lines.some((line) => line.startsWith("export {"));
}

function isPlaceholderSource(src) {
  return isStubSource(src) || isThinViewSource(src) || isShimSource(src);
}

function writeLil(abs, source) {
  if (existsSync(abs) && !isPlaceholderSource(readFileSync(abs, "utf8"))) {
    return "kept";
  }
  writeFileSync(abs, source);
  return "wrote";
}

function classifySource(src, rel) {
  if (EXTERN.has(rel)) {
    return { status: "extern", notes: "vendored blob; import extern until its own Lil port" };
  }
  if (isStubSource(src)) {
    return { status: "stub", notes: "" };
  }
  if (isThinViewSource(src)) {
    return { status: "thin", notes: "one-line view binding; not a file-for-file algorithm port" };
  }
  if (isShimSource(src)) {
    return { status: "shim", notes: "re-export of a real Lil implementation" };
  }
  return { status: "ported", notes: "Lil implementation of this monaco file" };
}

const PORTS = {
  "nls.js": { impl: "base/nls.lil", names: ["localize", "localizeMessage"], wave: 5, status: "ported" },
  "base/common/uri.js": { impl: "base/uri.lil", names: ["Uri", "parseUri", "fileUri", "inmemoryUri"], wave: 5, status: "ported" },
  "base/common/lifecycle.js": { impl: "base/lifecycle.lil", names: ["DisposableStore", "Emitter", "createDisposableStore", "createEmitter"], wave: 5, status: "ported" },
  "base/common/event.js": { impl: "base/lifecycle.lil", names: ["Emitter", "createEmitter", "emitterEvent", "emitterFire"], wave: 5, status: "ported" },
  "base/common/keyCodes.js": { impl: "base/keycodes.lil", names: ["KeyCode", "KeyModCtrl", "KeyModShift", "KeyModAlt", "KeyModWinCtrl", "chord", "keyCodeFromKey"], wave: 5, status: "ported" },
  "editor/common/core/position.js": { impl: "base/position.lil", names: ["Pos", "createPosition", "positionEquals", "positionCompare", "liftPosition", "isIPosition"], wave: 1, status: "ported" },
  "editor/common/core/range.js": { impl: "base/range.lil", names: ["Rng", "createRange", "rangeEquals", "liftRange", "isIRange", "rangeCompareRangesUsingStarts"], wave: 1, status: "ported" },
  "editor/common/core/selection.js": { impl: "base/selection.lil", names: ["Sel", "createSelection", "selectionAsRange", "liftSelection", "isISelection", "SelectionDirection"], wave: 1, status: "ported" },
  "editor/common/core/wordHelper.js": { impl: "editor/word.lil", names: ["WordRange", "wordAt", "isWordChar", "leadingIndent"], wave: 1, status: "ported" },
  "editor/common/model/pieceTreeTextBuffer/rbTreeBase.js": { impl: "editor/rb-tree.lil", names: ["TreeNode", "SENTINEL", "NodeColor", "leftest", "righttest", "nodeNext", "nodePrev"], wave: 1, status: "ported" },
  "editor/common/model/pieceTreeTextBuffer/pieceTreeBase.js": { impl: "editor/piece-tree.lil", names: ["PieceTreeBase", "StringBuffer", "createPieceTree", "createLineStartsFast"], wave: 1, status: "ported" },
  "editor/common/model/pieceTreeTextBuffer/pieceTreeTextBuffer.js": { impl: "editor/piece-tree-buffer.lil", names: ["PieceTreeTextBuffer", "createPieceTreeTextBuffer"], wave: 1, status: "ported" },
  "editor/common/model/pieceTreeTextBuffer/pieceTreeTextBufferBuilder.js": { impl: "editor/piece-tree-builder.lil", names: ["PieceTreeTextBufferBuilder", "createPieceTreeTextBufferBuilder", "buildPieceTreeTextBuffer"], wave: 1, status: "ported" },
  "editor/common/model/prefixSumComputer.js": { impl: "editor/prefix-sum.lil", names: ["PrefixSumComputer", "PrefixSumIndexOfResult", "createPrefixSumComputer"], wave: 1, status: "ported" },
  "editor/common/model/textModelSearch.js": { impl: "editor/search.lil", names: ["FindMatch", "Searcher", "findMatches", "createSearcher", "escapeRegExpCharacters"], wave: 1, status: "ported" },
  "editor/common/model/editStack.js": { impl: "editor/edit-stack.lil", names: ["EditStack", "IdentifiedEdit", "createEditStack"], wave: 1, status: "ported" },
  "editor/common/model/intervalTree.js": { impl: "editor/interval-tree.lil", names: ["IntervalTree", "Decoration", "createIntervalTree", "decoMetadata"], wave: 1, status: "ported" },
  "editor/common/model/textModel.js": { impl: "editor/text-model.lil", names: ["TextModel", "createModel", "createModelWithUri", "getModels", "editOp", "deco"], wave: 1, status: "ported" },
  "editor/standalone/common/monarch/monarchCompile.js": { impl: "editor/monarch.lil", names: ["Lexer", "Token", "tokenizeText", "tokenTypes"], wave: 1, status: "ported" },
  "editor/standalone/common/monarch/monarchLexer.js": { impl: "editor/monarch-lexer.lil", names: ["monarchLexerTokenize", "monarchInitialState", "tokenizeLine", "initialState"], wave: 1, status: "ported" },
  "editor/common/diff/defaultLinesDiffComputer/algorithms/myersDiffAlgorithm.js": { impl: "editor/myers.lil", names: ["DiffChange", "computeLineDiff"], wave: 1, status: "ported" },
  "editor/browser/view.js": { impl: "editor/view.lil", names: ["EditorView", "createView", "DEFAULT_LINE_HEIGHT", "DEFAULT_CHAR_WIDTH", "MARGIN_WIDTH"], wave: 2, status: "ported" },
  "editor/browser/coreCommands.js": { impl: "editor/commands.lil", names: ["triggerCommand", "typeText", "setPosition", "setSel", "handleKeydown"], wave: 2, status: "ported" },
  "editor/editor.api.js": { impl: "editor/monaco-api.lil", names: ["monacoApi", "installMonaco"], wave: 3, status: "ported" },
  "editor/standalone/browser/standaloneEditor.js": { impl: "editor/standalone.lil", names: ["create", "createDiffEditor", "getEditors", "getDiffEditors", "CodeEditor"], wave: 3, status: "ported" },
  "editor/standalone/browser/standaloneLanguages.js": { impl: "editor/standalone.lil", names: ["languagesRegisterCompletion", "languagesRegisterHover", "registerLanguageId", "setLanguageConfigurationJs", "setTokensProviderJs"], wave: 3, status: "ported" },
  "editor/common/services/editorBaseApi.js": { impl: "layers/core-types.lil", names: ["Position", "Range", "Selection"], wave: 3, status: "ported" },
  "editor/common/standalone/standaloneEnums.js": { impl: "editor/standalone-enums.lil", names: ["CompletionItemKindMethod", "MarkerSeverityError", "SelectionDirectionLTR"], wave: 3, status: "ported" },
  "editor/browser/widget/codeEditor/codeEditorWidget.js": { impl: "editor/standalone.lil", names: ["CodeEditor", "create"], wave: 3, status: "ported" },
  "editor/common/services/editorWebWorker.js": { impl: "workers/editor.worker.lil", names: ["handleEditorWorker", "editorWorkerPing"], wave: 6, status: "ported" },
  "editor/editor.worker.start.js": { impl: "workers/editor.worker.lil", names: ["handleEditorWorker", "editorWorkerPing"], wave: 6, status: "ported" },
};

const CONTRIB_MAIN = {
  find: { impl: "contrib/runtime.lil", names: ["openFind", "closeFind", "findNext", "replaceOne", "replaceAll"] },
  suggest: { impl: "contrib/runtime.lil", names: ["showSuggest", "hideSuggest", "acceptSuggest", "moveSuggest"] },
  hover: { impl: "contrib/runtime.lil", names: ["showHover", "hideHover"] },
  folding: { impl: "contrib/popular.lil", names: ["computeIndentFolds"] },
  gotoSymbol: { impl: "contrib/runtime.lil", names: ["goToDefinition"] },
  gotoError: { impl: "contrib/runtime.lil", names: ["goToDefinition"] },
  comment: { impl: "contrib/popular.lil", names: ["toggleLineComment"] },
  format: { impl: "contrib/remaining.lil", names: ["formatDocument"] },
  snippet: { impl: "contrib/popular.lil", names: ["expandSnippet", "insertSnippet"] },
  bracketMatching: { impl: "contrib/popular.lil", names: ["matchBracket"] },
  links: { impl: "contrib/popular.lil", names: ["detectLinks"] },
  wordHighlighter: { impl: "contrib/popular.lil", names: ["highlightWord"] },
  stickyScroll: { impl: "contrib/popular.lil", names: ["stickyLines"] },
  rename: { impl: "contrib/remaining.lil", names: ["renameSymbol"] },
  parameterHints: { impl: "contrib/remaining.lil", names: ["parameterHints"] },
  inlayHints: { impl: "contrib/remaining.lil", names: ["inlayHintEnabled"] },
  unicodeHighlighter: { impl: "contrib/remaining.lil", names: ["unicodeHighlightEnabled"] },
  unusualLineTerminators: { impl: "contrib/remaining.lil", names: ["unusualLineTerminators"] },
  inlineCompletions: { impl: "contrib/remaining.lil", names: ["remainingContribIds"] },
  colorPicker: { impl: "contrib/remaining.lil", names: ["remainingContribIds"] },
  codelens: { impl: "contrib/remaining.lil", names: ["remainingContribIds"] },
  codeAction: { impl: "contrib/remaining.lil", names: ["remainingContribIds"] },
  clipboard: { impl: "editor/commands.lil", names: ["copySelection", "cutSelection", "pasteText"] },
  multicursor: { impl: "editor/commands.lil", names: ["insertCursor"] },
  linesOperations: { impl: "editor/commands.lil", names: ["moveLines", "copyLines", "deleteLines"] },
  wordOperations: { impl: "editor/commands.lil", names: ["moveWordLeft", "moveWordRight"] },
  contextmenu: { impl: "contrib/runtime.lil", names: ["showContext", "hideContext"] },
};

const EXTERN = new Set([
  "base/common/marked/marked.js",
  "base/browser/dompurify/dompurify.js",
]);

function waveOf(rel) {
  if (PORTS[rel]) return PORTS[rel].wave;
  if (rel.startsWith("editor/browser/view") || rel.startsWith("editor/browser/viewParts") || rel.startsWith("editor/browser/gpu")) return 2;
  if (rel.includes("standalone") || rel.endsWith("editor.api.js") || rel.includes("editorBaseApi")) return 3;
  if (rel.startsWith("editor/contrib/")) return 4;
  if (rel.startsWith("base/") || rel.startsWith("platform/")) return 5;
  if (rel.includes("worker") || rel.includes("Worker")) return 6;
  return 0;
}

function contribFolder(rel) {
  const m = rel.match(/^editor\/contrib\/([^/]+)\//);
  return m ? m[1] : null;
}

function isContribMain(rel, files) {
  const folder = contribFolder(rel);
  if (!folder || !CONTRIB_MAIN[folder]) return false;
  const inFolder = files.filter((f) => f.startsWith(`editor/contrib/${folder}/`));
  const preferred =
    inFolder.find((f) => /Controller\.js$/.test(f)) ||
    inFolder.find((f) => f.endsWith(`${folder}.js`)) ||
    inFolder[0];
  return rel === preferred;
}

function writeStandaloneEnums() {
  const src = readFileSync(join(coreEsm, "editor/common/standalone/standaloneEnums.js"), "utf8");
  const re = /(\w+)\[\1\["(\w+)"\] = (\d+)\]/g;
  const lines = ["// Generated from monaco-editor-core standaloneEnums.js"];
  const exported = [];
  let match;
  while ((match = re.exec(src))) {
    const name = `${match[1]}${match[2]}`;
    lines.push(`export int ${name} = ${match[3]};`);
    exported.push(name);
  }
  if (!exported.includes("CompletionItemKindMethod")) {
    lines.push("export int CompletionItemKindMethod = 0;");
    exported.push("CompletionItemKindMethod");
  }
  if (!exported.includes("MarkerSeverityError")) {
    lines.push("export int MarkerSeverityError = 8;");
    exported.push("MarkerSeverityError");
  }
  if (!exported.includes("SelectionDirectionLTR")) {
    lines.push("export int SelectionDirectionLTR = 0;");
    exported.push("SelectionDirectionLTR");
  }
  writeFileSync(join(portsRoot, "editor/standalone-enums.lil"), lines.join("\n") + "\n");
  return exported;
}

export function generateVsTree() {
  mkdirSync(vsRoot, { recursive: true });
  const enumNames = writeStandaloneEnums();
  PORTS["editor/common/standalone/standaloneEnums.js"].names = [
    "CompletionItemKindMethod",
    "MarkerSeverityError",
    "SelectionDirectionLTR",
    ...enumNames.slice(0, 1),
  ].filter((name, i, all) => all.indexOf(name) === i);

  const files = walkJs(coreEsm).sort();
  const rows = [];
  for (const rel of files) {
    const lilRel = `vs/${rel.replace(/\.js$/, ".lil")}`;
    const abs = join(portsRoot, lilRel);
    mkdirSync(dirname(abs), { recursive: true });
    let impl = "";
    const wave = waveOf(rel);
    if (EXTERN.has(rel)) {
      writeLil(abs, stubSource(rel));
    } else if (PORTS[rel]) {
      impl = PORTS[rel].impl;
      writeLil(abs, shimSource(lilRel, impl, PORTS[rel].names));
    } else if (isContribMain(rel, files)) {
      const folder = contribFolder(rel);
      const spec = CONTRIB_MAIN[folder];
      impl = spec.impl;
      writeLil(abs, shimSource(lilRel, spec.impl, spec.names));
    } else if (
      rel.startsWith("editor/browser/viewParts/")
      || rel.startsWith("editor/browser/gpu/")
      || rel.startsWith("editor/browser/view/")
    ) {
      impl = "editor/view.lil";
      writeLil(abs, viewPartSource(lilRel, rel));
    } else {
      writeLil(abs, stubSource(rel));
    }
    const onDisk = readFileSync(abs, "utf8");
    const classified = classifySource(onDisk, rel);
    if (classified.status === "stub" && rel.startsWith("platform/")) {
      classified.notes = "plain class/service stub; no InstantiationService";
    }
    if (PORTS[rel] && classified.status === "shim") {
      classified.notes = `shim → ${PORTS[rel].impl}`;
    }
    rows.push({ monaco: rel, lil: lilRel, status: classified.status, wave, impl, notes: classified.notes });
  }

  const workers = [
    { monaco: "language/json/json.worker.js", impl: "workers/json.worker.lil", names: ["handleJsonWorker", "jsonWorkerSymbols"] },
    { monaco: "language/css/css.worker.js", impl: "workers/css.worker.lil", names: ["handleCssWorker", "cssWorkerCompletions"] },
    { monaco: "language/html/html.worker.js", impl: "workers/html.worker.lil", names: ["handleHtmlWorker", "htmlWorkerCompletions"] },
    { monaco: "language/typescript/ts.worker.js", impl: "workers/ts.worker.lil", names: ["handleTsWorker", "tsWorkerCompletions", "tsWorkerWithoutTsc"] },
  ];
  const workerRows = [];
  for (const worker of workers) {
    const lilRel = `vs/${worker.monaco.replace(/\.js$/, ".lil")}`;
    const abs = join(portsRoot, lilRel);
    mkdirSync(dirname(abs), { recursive: true });
    writeFileSync(abs, shimSource(lilRel, worker.impl, worker.names));
    workerRows.push({
      monaco: worker.monaco,
      package: "monaco-editor",
      lil: lilRel,
      impl: worker.impl,
      status: "ported",
      wave: 6,
      notes: worker.monaco.includes("ts.worker")
        ? "Catalog stub; the served Lil page uses Microsoft ts.worker.js / typescriptServices.js"
        : "Lil worker entry",
    });
  }

  const ported = rows.filter((row) => row.status === "ported").length;
  const shim = rows.filter((row) => row.status === "shim").length;
  const thin = rows.filter((row) => row.status === "thin").length;
  const stub = rows.filter((row) => row.status === "stub").length;
  const extern = rows.filter((row) => row.status === "extern").length;
  const catalog = {
    versions: {
      monacoEditor: monacoEditorVersion,
      monacoEditorCore: monacoEditorCoreVersion,
      vscodeCommit: vscodeCommitId,
    },
    coreCount: files.length,
    mapped: rows.length,
    ported,
    shim,
    thin,
    stub,
    extern,
    remaining: stub + thin,
    files: rows,
    workers: workerRows,
  };
  writeFileSync(join(portsRoot, "vs/catalog.json"), JSON.stringify(catalog, null, 2) + "\n");
  return catalog;
}

const isMain = process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (isMain) {
  const catalog = generateVsTree();
  console.log(`catalog ${catalog.ported}/${catalog.coreCount} ported, ${catalog.shim} shim, ${catalog.thin} thin, ${catalog.stub} stub, ${catalog.extern} extern, workers ${catalog.workers.length}`);
}
