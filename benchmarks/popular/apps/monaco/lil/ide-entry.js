import { bootMonaco } from "../../../build/monaco-layers/entry.raw.js";
import { attachTypescript } from "../../../ports/monaco/ts-language/attach-typescript.js";
import { mountIde } from "../workbench.js";

const monaco = bootMonaco();
globalThis.monaco = monaco;
globalThis.__lilEditor = true;

await attachTypescript(monaco);

mountIde(monaco, {
  label: "LilScript monaco",
  otherHref: "../js/",
  otherLabel: "JS monaco-editor →",
  languageFeatures: true,
  banner:
    "Compiled LilScript monaco plus the official Microsoft TypeScript worker (typescriptServices.js). The editor API is LilScript; tsc runs in ts.worker.js.",
});
