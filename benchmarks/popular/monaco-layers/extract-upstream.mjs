import { mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { build as esbuild } from "esbuild";
import { layerById, monacoEditorCoreVersion } from "./catalog.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const labRoot = join(here, "..");
const coreRoot = join(labRoot, "node_modules/monaco-editor-core/esm/vs");
const monacoLangRoot = join(labRoot, "node_modules/monaco-editor/esm/vs/languages/definitions");

const wrappers = {
  "base-lifecycle": `
export { Disposable, DisposableStore, toDisposable } from "${coreRoot}/base/common/lifecycle.js";
export { Emitter } from "${coreRoot}/base/common/event.js";
export { URI } from "${coreRoot}/base/common/uri.js";
export { KeyCode } from "${coreRoot}/editor/common/standalone/standaloneEnums.js";
export { KeyCodeUtils } from "${coreRoot}/base/common/keyCodes.js";
`,
  "core-types": `
export { Position } from "${coreRoot}/editor/common/core/position.js";
export { Range } from "${coreRoot}/editor/common/core/range.js";
export { Selection } from "${coreRoot}/editor/common/core/selection.js";
`,
  "piece-tree": `
import { PieceTreeTextBufferBuilder } from "${coreRoot}/editor/common/model/pieceTreeTextBuffer/pieceTreeTextBufferBuilder.js";
import { Range } from "${coreRoot}/editor/common/core/range.js";

export function create(value, eol = "\\n") {
  const builder = new PieceTreeTextBufferBuilder();
  if (value) builder.acceptChunk(value);
  const factory = builder.finish(true);
  return factory.create(eol === "\\r\\n" ? 2 : 1).textBuffer;
}
export function getValue(buf) {
  return buf.getLinesContent().join(buf.getEOL());
}
export function getLength(buf) { return buf.getLength(); }
export function getLineCount(buf) { return buf.getLineCount(); }
export function getLineContent(buf, lineNumber) { return buf.getLineContent(lineNumber); }
export function getLineLength(buf, lineNumber) { return buf.getLineLength(lineNumber); }
export function getOffsetAt(buf, lineNumber, column) { return buf.getOffsetAt(lineNumber, column); }
export function getPositionAt(buf, offset) { return buf.getPositionAt(offset); }
export function getValueInRange(buf, sl, sc, el, ec) {
  return buf.getValueInRange(new Range(sl, sc, el, ec));
}
export function insert(buf, offset, value) { buf._pieceTree.insert(offset, value); }
export function deleteRange(buf, offset, cnt) { buf._pieceTree.delete(offset, cnt); }
`,
  "text-model": `
import { PieceTreeTextBufferBuilder } from "${coreRoot}/editor/common/model/pieceTreeTextBuffer/pieceTreeTextBufferBuilder.js";
import { Range } from "${coreRoot}/editor/common/core/range.js";
import { IntervalTree, IntervalNode, SENTINEL } from "${coreRoot}/editor/common/model/intervalTree.js";
import { Searcher, createFindMatch } from "${coreRoot}/editor/common/model/textModelSearch.js";

export function createModel(value) {
  const builder = new PieceTreeTextBufferBuilder();
  if (value) builder.acceptChunk(value);
  const factory = builder.finish(true);
  const buf = factory.create(1).textBuffer;
  return {
    buf,
    getValue() { return buf.getLinesContent().join(buf.getEOL()); },
    getLineCount() { return buf.getLineCount(); },
    applyEdits(edits) {
      const ops = edits.map((e) => ({ range: new Range(e.range.startLineNumber, e.range.startColumn, e.range.endLineNumber, e.range.endColumn), text: e.text }));
      buf.applyEdits(ops, false, false);
    },
  };
}
export { IntervalTree, Searcher, Range };
`,
  "view-render": `
export { View } from "${coreRoot}/editor/browser/view.js";
`,
  "input-commands": `
export { View } from "${coreRoot}/editor/browser/view.js";
export { Position } from "${coreRoot}/editor/common/core/position.js";
`,
  "standalone-api": `
export { Position, Range, Selection, editor, Uri } from "${coreRoot}/editor/editor.api.js";
`,
  "monarch-popular": `
export { compile } from "${coreRoot}/editor/standalone/common/monarch/monarchCompile.js";
import { language as javascript } from "${monacoLangRoot}/javascript/javascript.js";
import { language as python } from "${monacoLangRoot}/python/python.js";
import { language as html } from "${monacoLangRoot}/html/html.js";
import { language as css } from "${monacoLangRoot}/css/css.js";
import { language as markdown } from "${monacoLangRoot}/markdown/markdown.js";
import { language as typescript } from "${monacoLangRoot}/typescript/typescript.js";
export const languages = { javascript, python, html, css, markdown, typescript };
`,
  "popular-contrib": `
export { editor, Range, Position } from "${coreRoot}/editor/editor.api.js";
`,
  "remaining-contrib": `
export { editor } from "${coreRoot}/editor/editor.api.js";
`,
  "json-css-html-ls": `
export { languages } from "${coreRoot}/editor/editor.api.js";
`,
};

function remainingMonarchWrapper() {
  const popular = new Set(["javascript", "typescript", "json", "html", "css", "python", "markdown"]);
  const dirs = readdirSync(monacoLangRoot, { withFileTypes: true }).filter((d) => d.isDirectory() && !popular.has(d.name));
  const imports = [];
  const names = [];
  for (const dir of dirs) {
    const ident = dir.name.replace(/[^a-zA-Z0-9]/g, "_");
    imports.push(`import * as ${ident} from "${monacoLangRoot}/${dir.name}/${dir.name}.js";`);
    names.push(ident);
  }
  return `${imports.join("\n")}
export const remaining = { ${names.join(", ")} };
`;
}

export async function extractLayer(id) {
  const layer = layerById(id);
  const source = id === "remaining-monarch" ? remainingMonarchWrapper() : wrappers[layer.extract];
  if (!source) {
    throw new Error(`no extract wrapper for ${id}`);
  }
  const result = await esbuild({
    stdin: {
      contents: `// Extracted from monaco-editor-core ${monacoEditorCoreVersion} for layer ${id}.\n${source}`,
      resolveDir: labRoot,
      sourcefile: `${id}.extract.js`,
      loader: "js",
    },
    bundle: true,
    format: "esm",
    platform: "neutral",
    write: false,
    logOverride: {
      "import-is-undefined": "silent",
      "empty-import-meta": "silent",
    },
    plugins: [
      {
        name: "empty-assets",
        setup(build) {
          build.onLoad({ filter: /\.(css|ttf|woff2?|svg)$/ }, () => ({
            contents: "export default ''",
            loader: "js",
          }));
        },
      },
    ],
  });
  return result.outputFiles[0].text;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const id = process.argv[2];
  if (!id) {
    throw new Error("usage: node extract-upstream.mjs <layer-id>");
  }
  process.stdout.write(await extractLayer(id));
}
