import { readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const labRoot = join(here, "..");
export const coreEsm = join(labRoot, "node_modules/monaco-editor-core/esm/vs");
export const monacoEsm = join(labRoot, "node_modules/monaco-editor/esm/vs");
export const portsRoot = join(labRoot, "ports/monaco");

export const pairs = [
  {
    id: "position",
    title: "Position",
    plugged: false,
    measure: true,
    monacoFiles: ["editor/common/core/position.js"],
    lilFiles: ["base/position.lil"],
    lilEntry: "base/position.lil",
    jsEntry: "editor/common/core/position.js",
    jsExternal: [],
    note: "Single file. Lil class is Pos; monaco statics (equals, lift) are not a plug facade yet.",
  },
  {
    id: "range",
    title: "Range",
    plugged: false,
    measure: false,
    monacoFiles: ["editor/common/core/range.js"],
    lilFiles: ["base/range.lil"],
    lilEntry: "base/range.lil",
    jsEntry: "editor/common/core/range.js",
    jsExternal: [/\/position\.js$/],
    note: "JS keeps Position as an import. Lil inlines Pos.",
  },
  {
    id: "selection",
    title: "Selection",
    plugged: false,
    measure: false,
    monacoFiles: ["editor/common/core/selection.js"],
    lilFiles: ["base/selection.lil"],
    lilEntry: "base/selection.lil",
    jsEntry: "editor/common/core/selection.js",
    jsExternal: [/\/position\.js$/, /\/range\.js$/],
    note: "monaco Selection extends Range. Lil Sel does not; not plugged.",
  },
  {
    id: "core-types",
    title: "Position + Range + Selection",
    plugged: false,
    measure: true,
    monacoFiles: [
      "editor/common/core/position.js",
      "editor/common/core/range.js",
      "editor/common/core/selection.js",
    ],
    lilFiles: ["base/position.lil", "base/range.lil", "base/selection.lil", "layers/core-types.lil"],
    lilEntry: "layers/core-types.lil",
    jsWrapper: `export { Position } from "${coreEsm}/editor/common/core/position.js";
export { Range } from "${coreEsm}/editor/common/core/range.js";
export { Selection } from "${coreEsm}/editor/common/core/selection.js";
`,
    jsExternal: [],
    note: "The three files monaco actually ships. One Lil compile of the same three.",
  },
  {
    id: "piece-tree",
    title: "PieceTreeBase + rbTreeBase",
    plugged: true,
    measure: true,
    monacoFiles: [
      "editor/common/model/pieceTreeTextBuffer/pieceTreeBase.js",
      "editor/common/model/pieceTreeTextBuffer/rbTreeBase.js",
    ],
    lilFiles: ["editor/rb-tree.lil", "editor/piece-tree.lil", "layers/piece-tree.lil"],
    lilEntry: "layers/piece-tree.lil",
    jsEntry: "editor/common/model/pieceTreeTextBuffer/pieceTreeBase.js",
    jsExternal: [
      /\/position\.js$/,
      /\/range\.js$/,
      /\/model\.js$/,
      /\/textModelSearch\.js$/,
    ],
    plugFile: "plug/piece-tree-base.js",
    note: "Fair pair is pieceTreeBase + rbTreeBase. Buffer and builder are separate Lil files under vs/editor/common/model/pieceTreeTextBuffer/. findMatchesLineByLine is in Lil PieceTreeBase.",
  },
  {
    id: "piece-tree-buffer",
    title: "PieceTreeTextBuffer + Builder",
    plugged: true,
    measure: false,
    monacoFiles: [
      "editor/common/model/pieceTreeTextBuffer/pieceTreeTextBuffer.js",
      "editor/common/model/pieceTreeTextBuffer/pieceTreeTextBufferBuilder.js",
    ],
    lilFiles: ["editor/piece-tree-buffer.lil", "editor/piece-tree-builder.lil"],
    lilEntry: "editor/piece-tree-builder.lil",
    jsEntry: "editor/common/model/pieceTreeTextBuffer/pieceTreeTextBufferBuilder.js",
    jsExternal: [
      /\/position\.js$/,
      /\/range\.js$/,
      /\/model\.js$/,
      /\/pieceTreeBase\.js$/,
      /\/strings\.js$/,
    ],
    note: "Lil sources exist. Not scored here because the Lil compile of the wrapper also contains PieceTreeBase, while the JS side leaves pieceTreeBase external.",
  },
  {
    id: "uri",
    title: "URI",
    plugged: false,
    measure: false,
    monacoFiles: ["base/common/uri.js"],
    lilFiles: ["base/uri.lil"],
    lilEntry: "base/uri.lil",
    jsEntry: "base/common/uri.js",
    jsExternal: [/\/path\.js$/, /\/platform\.js$/],
    note: "Lil URI is the parse/file/inmemory subset, not win32/posix vscode URI.",
  },
  {
    id: "lifecycle",
    title: "Disposable + Emitter",
    plugged: false,
    measure: false,
    monacoFiles: ["base/common/lifecycle.js", "base/common/event.js"],
    lilFiles: ["base/lifecycle.lil"],
    lilEntry: "base/lifecycle.lil",
    jsWrapper: `export { Disposable, DisposableStore, toDisposable } from "${coreEsm}/base/common/lifecycle.js";
export { Emitter } from "${coreEsm}/base/common/event.js";
`,
    jsExternal: [/\/iterator\.js$/, /\/errors\.js$/, /\/functional\.js$/, /\/linkedList\.js$/, /\/process\.js$/, /\/stopwatch\.js$/],
    note: "Two monaco files. Emitter lives in event.js. Not the full vscode event service.",
  },
  {
    id: "interval-tree",
    title: "Decoration interval tree",
    plugged: false,
    measure: true,
    monacoFiles: ["editor/common/model/intervalTree.js"],
    lilFiles: ["editor/interval-tree.lil"],
    lilEntry: "editor/interval-tree.lil",
    jsEntry: "editor/common/model/intervalTree.js",
    jsExternal: [],
    note: "Ported algorithm. Lil Decoration.metadata is the unpacked equivalent of monaco's packed node metadata bitfields.",
  },
  {
    id: "myers",
    title: "Myers diff",
    plugged: false,
    measure: true,
    monacoFiles: ["editor/common/diff/defaultLinesDiffComputer/algorithms/myersDiffAlgorithm.js"],
    lilFiles: ["editor/myers.lil"],
    lilEntry: "editor/myers.lil",
    jsEntry: "editor/common/diff/defaultLinesDiffComputer/algorithms/myersDiffAlgorithm.js",
    jsExternal: [/\/offsetRange\.js$/, /\/diffAlgorithm\.js$/],
    note: "Algorithm file only. monaco's defaultLinesDiffComputer graph is not in this pair.",
  },
  {
    id: "monarch",
    title: "Monarch compile",
    plugged: false,
    measure: true,
    monacoFiles: ["editor/standalone/common/monarch/monarchCompile.js"],
    lilFiles: ["editor/monarch.lil"],
    lilEntry: "editor/monarch.lil",
    jsEntry: "editor/standalone/common/monarch/monarchCompile.js",
    jsExternal: [/\/monarchCommon\.js$/],
    note: "Compile file. monarchLexer.js is Lil in editor/monarch-lexer.lil.",
  },
  {
    id: "search",
    title: "Text model search",
    plugged: false,
    measure: false,
    monacoFiles: ["editor/common/model/textModelSearch.js"],
    lilFiles: ["editor/search.lil"],
    lilEntry: "editor/search.lil",
    jsEntry: "editor/common/model/textModelSearch.js",
    jsExternal: [/\/position\.js$/, /\/range\.js$/, /\/model\.js$/, /\/wordHelper\.js$/],
    note: "Ported. findMatchesLineByLine lives on Lil PieceTreeBase; Searcher is in search.lil.",
  },
  {
    id: "edit-stack",
    title: "Undo stack",
    plugged: false,
    measure: false,
    monacoFiles: ["editor/common/model/editStack.js"],
    lilFiles: ["editor/edit-stack.lil"],
    lilEntry: "editor/edit-stack.lil",
    jsEntry: "editor/common/model/editStack.js",
    jsExternal: [/\/errors\.js$/, /\/nls\.js$/, /\/uri\.js$/],
    note: "Ported. TextModel uses the Lil edit stack.",
  },
];

export const notOneToOne = [];

export function countJsFiles(dir) {
  let n = 0;
  for (const ent of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, ent.name);
    if (ent.isDirectory()) {
      n += countJsFiles(path);
    } else if (ent.name.endsWith(".js")) {
      n += 1;
    }
  }
  return n;
}

export function monacoPath(rel) {
  return join(coreEsm, rel);
}

export function lilPath(rel) {
  return join(portsRoot, rel);
}

export function pairById(id) {
  const pair = pairs.find((entry) => entry.id === id);
  if (!pair) {
    throw new Error(`unknown file-map pair ${id}`);
  }
  return pair;
}
