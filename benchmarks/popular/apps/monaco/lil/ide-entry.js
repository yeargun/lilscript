import { bindMonaco } from "../../../ports/monaco/monaco-api.ts";
import * as lil from "../../../build/monaco-layers/entry.raw.js";
import { mountIde } from "../workbench.js";

const monaco = bindMonaco(lil);
globalThis.monaco = monaco;

mountIde(monaco, {
  label: "LilScript",
  otherHref: "../js/",
  otherLabel: "JS monaco-editor →",
  banner:
    "LilScript compiled editor: piece-tree model, Monarch highlighting, textarea + canvas minimap. Not VS Code workbench, not tsc/ts.worker.",
});
