import "./monaco-env.js";
import * as monaco from "monaco-editor";
import { mountIde } from "../workbench.js";

globalThis.monaco = monaco;

mountIde(monaco, {
  label: "monaco-editor 0.56",
  otherHref: "../lil/",
  otherLabel: "← LilScript",
  languageFeatures: true,
  banner:
    "Production monaco-editor 0.56 (esbuild minify, Brotli on the wire). This is the npm JavaScript editor. The other page is the LilScript port of the same product.",
});
