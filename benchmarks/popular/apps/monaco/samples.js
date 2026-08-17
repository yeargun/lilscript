export const WORKSPACE = "demo-ide";

export const FILES = [
  {
    path: "README.md",
    language: "markdown",
    value: `# demo-ide

Two served editors, same chrome:

- LilScript: compiled Lil editor (piece tree, Monarch, textarea + minimap canvas)
- JS: npm monaco-editor 0.56 (VS Code editor + JSON/CSS/HTML/TS workers)

Open files from the explorer. Ctrl/Cmd+P quick-opens. Ctrl/Cmd+F finds in the current file.
`,
  },
  {
    path: "src/main.ts",
    language: "typescript",
    value: `import { greet, add } from "./app";

const root = document.getElementById("app");
if (root) {
  root.textContent = greet("Monaco");
}

export function boot(name: string): number {
  return add(name.length, 1);
}
`,
  },
  {
    path: "src/app.ts",
    language: "typescript",
    value: `export function greet(name: string): string {
  return "hello, " + name;
}

export function add(left: number, right: number): number {
  return left + right;
}

export const sample: number = greet("world");
`,
  },
  {
    path: "src/styles.css",
    language: "css",
    value: `:root {
  color-scheme: dark;
  --bg: #1e1e1e;
  --fg: #d4d4d4;
  --accent: #007acc;
}

body {
  margin: 0;
  background: var(--bg);
  color: var(--fg);
  font-family: "Segoe UI", sans-serif;
}

#app {
  padding: 24px;
}
`,
  },
  {
    path: "index.html",
    language: "html",
    value: `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>demo-ide</title>
    <link rel="stylesheet" href="./src/styles.css" />
  </head>
  <body>
    <div id="app">loading</div>
    <script type="module" src="./src/main.ts"></script>
  </body>
</html>
`,
  },
  {
    path: "package.json",
    language: "json",
    value: `{
  "name": "demo-ide",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "echo open the served editor"
  }
}
`,
  },
  {
    path: "data.json",
    language: "json",
    value: `{
  "editor": "monaco",
  "versions": ["0.56.0"],
  "features": ["models", "tabs", "find", "markers"]
}
`,
  },
];
