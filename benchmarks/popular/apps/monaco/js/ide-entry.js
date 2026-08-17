import "./monaco-env.js";
import * as monaco from "monaco-editor";
import { mountIde } from "../workbench.js";

mountIde(monaco, {
  label: "monaco-editor 0.56",
  otherHref: "../lil/",
  otherLabel: "← LilScript",
  banner:
    "npm monaco-editor 0.56 — VS Code editor, GPU view, suggest, and JSON/CSS/HTML/TypeScript workers. This is the real package, not a subset.",
});
