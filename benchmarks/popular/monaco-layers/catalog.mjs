export const monacoEditorVersion = "0.56.0";
export const monacoEditorCoreVersion = "0.56.0";
export const vscodeCommitId = "f487add297079a02eb836810185b165e50cadabc";
export const monacoEditorCommitId = "13f0c872dcf352815cc28d92dfff496c9839ea5c";

export const planned = [
  "base-lifecycle",
  "core-types",
  "piece-tree",
  "text-model",
  "view-render",
  "input-commands",
  "standalone-api",
  "monarch-popular",
  "popular-contrib",
  "remaining-contrib",
  "remaining-monarch",
  "json-css-html-ls",
];

export const layers = [
  {
    id: "base-lifecycle",
    title: "Disposable, Emitter, URI, and key codes",
    dependsOn: [],
    exports: ["createDisposableStore", "createEmitter", "parseUri", "fileUri", "inmemoryUri", "KeyCode", "chord", "keyCodeFromKey"],
    lilEntry: "ports/monaco/layers/base-lifecycle.lil",
    verify: "monaco-layers/layers/base-lifecycle/verify.mjs",
    extract: "base-lifecycle",
  },
  {
    id: "core-types",
    title: "Position, Range, and Selection",
    dependsOn: ["base-lifecycle"],
    exports: ["Position", "Range", "Selection"],
    lilEntry: "ports/monaco/layers/core-types.lil",
    verify: "monaco-layers/layers/core-types/verify.mjs",
    extract: "core-types",
  },
  {
    id: "piece-tree",
    title: "Piece tree text buffer",
    dependsOn: ["core-types"],
    exports: ["create", "getValue", "insert", "deleteRange", "getLineCount", "getLineContent", "getLength", "getOffsetAt"],
    lilEntry: "ports/monaco/layers/piece-tree.lil",
    verify: "monaco-layers/layers/piece-tree/verify.mjs",
    extract: "piece-tree",
  },
  {
    id: "text-model",
    title: "TextModel, undo stack, decorations, search",
    dependsOn: ["piece-tree"],
    exports: ["createModel", "editOp"],
    lilEntry: "ports/monaco/layers/text-model.lil",
    verify: "monaco-layers/layers/text-model/verify.mjs",
    extract: "text-model",
  },
  {
    id: "view-render",
    title: "Viewport, view lines, minimap, theme",
    dependsOn: ["text-model"],
    exports: ["mountView", "createView"],
    lilEntry: "ports/monaco/layers/view-render.lil",
    verify: "monaco-layers/layers/view-render/verify.mjs",
    extract: "view-render",
  },
  {
    id: "input-commands",
    title: "Type, delete, move, undo commands",
    dependsOn: ["view-render"],
    exports: ["mountCommands", "typeText", "deleteLeft", "undoEdit", "triggerCommand"],
    lilEntry: "ports/monaco/layers/input-commands.lil",
    verify: "monaco-layers/layers/input-commands/verify.mjs",
    extract: "input-commands",
  },
  {
    id: "standalone-api",
    title: "monaco.editor.create facade",
    dependsOn: ["input-commands"],
    exports: ["create", "createModel", "defineTheme", "setTheme", "createDiffEditor"],
    lilEntry: "ports/monaco/layers/standalone-api.lil",
    verify: "monaco-layers/layers/standalone-api/verify.mjs",
    extract: "standalone-api",
  },
  {
    id: "monarch-popular",
    title: "Monarch tokenizer and popular languages",
    dependsOn: ["standalone-api"],
    exports: ["tokenize", "javascriptLexer", "jsonLexer", "pythonLexer"],
    lilEntry: "ports/monaco/layers/monarch-popular.lil",
    verify: "monaco-layers/layers/monarch-popular/verify.mjs",
    extract: "monarch-popular",
  },
  {
    id: "popular-contrib",
    title: "Find, fold, brackets, hover, suggest, snippets, comments, goto, diff",
    dependsOn: ["monarch-popular"],
    exports: ["findInEditor", "computeIndentFolds", "matchBracket", "hoverAt", "suggestAt", "computeLineDiff"],
    lilEntry: "ports/monaco/layers/popular-contrib.lil",
    verify: "monaco-layers/layers/popular-contrib/verify.mjs",
    extract: "popular-contrib",
  },
  {
    id: "remaining-contrib",
    title: "Remaining contrib registrations",
    dependsOn: ["popular-contrib"],
    exports: ["remainingContribIds", "formatDocument", "parameterHints"],
    lilEntry: "ports/monaco/layers/remaining-contrib.lil",
    verify: "monaco-layers/layers/remaining-contrib/verify.mjs",
    extract: "remaining-contrib",
  },
  {
    id: "remaining-monarch",
    title: "Remaining Monarch languages",
    dependsOn: ["monarch-popular"],
    exports: ["registerRemainingLanguages", "remainingLanguageIds", "allLanguageIds"],
    lilEntry: "ports/monaco/layers/remaining-monarch.lil",
    verify: "monaco-layers/layers/remaining-monarch/verify.mjs",
    extract: "remaining-monarch",
  },
  {
    id: "json-css-html-ls",
    title: "JSON/CSS/HTML language-service adapters without tsc",
    dependsOn: ["remaining-contrib", "remaining-monarch"],
    exports: ["jsonDocumentSymbols", "cssCompletions", "htmlCompletions", "languageServicesWithoutTsc"],
    lilEntry: "ports/monaco/layers/json-css-html-ls.lil",
    verify: "monaco-layers/layers/json-css-html-ls/verify.mjs",
    extract: "json-css-html-ls",
  },
];

export function layerById(id) {
  const layer = layers.find((entry) => entry.id === id);
  if (!layer) {
    throw new Error(
      `unknown monaco layer ${JSON.stringify(id)}; implemented: ${layers
        .map((entry) => entry.id)
        .join(", ")}; planned: ${planned.join(" → ")}`,
    );
  }
  return layer;
}
