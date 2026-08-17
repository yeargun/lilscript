import { bindMonaco } from "../../../ports/monaco/monaco-api.ts";
import * as lil from "../../../build/monaco-layers/entry.raw.js";
import { mountIde } from "../workbench.js";

const monaco = bindMonaco(lil);
globalThis.monaco = monaco;
globalThis.__lilEditor = true;

mountIde(monaco, {
  label: "LilScript monaco",
  otherHref: "../js/",
  otherLabel: "JS monaco-editor →",
  languageFeatures: false,
  banner:
    "This page is the LilScript monaco port (piece tree, model, view, commands, Monarch, contrib). No monaco-editor JavaScript is in the bundle.",
});
