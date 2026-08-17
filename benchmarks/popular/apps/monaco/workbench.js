import { FILES, WORKSPACE } from "./samples.js";

function fileUri(monaco, path) {
  const raw = "file:///" + WORKSPACE + "/" + path;
  if (monaco.Uri?.parse) {
    return monaco.Uri.parse(raw);
  }
  return { path, toString: () => raw };
}

function readPosition(editor) {
  const pos = editor.getPosition?.();
  if (pos && typeof pos.lineNumber === "number") {
    return pos;
  }
  const sel = editor.getSelection?.();
  if (sel && typeof sel.positionLineNumber === "number") {
    return { lineNumber: sel.positionLineNumber, column: sel.positionColumn };
  }
  return { lineNumber: 1, column: 1 };
}

function markerRows(monaco) {
  const rows = monaco.editor.getModelMarkers?.({}) ?? [];
  return rows.map((m) => ({
    message: m.message ?? "",
    severity: Number(m.severity ?? 8),
    startLineNumber: Number(m.startLineNumber ?? 1),
    startColumn: Number(m.startColumn ?? 1),
    resource: String(m.resource?.path ?? m.resource?.toString?.() ?? m.resource ?? ""),
  })).filter((m) => m.message);
}

function severityLabel(severity) {
  if (severity >= 8) return "error";
  if (severity >= 4) return "warning";
  if (severity >= 2) return "info";
  return "hint";
}

export function mountIde(monaco, options) {
  const root = document.getElementById("app");
  const hasTs = Boolean(monaco.languages?.typescript);
  root.innerHTML = `
    <div class="quick-open" id="quick-open">
      <input id="quick-input" placeholder="Go to file" autocomplete="off" />
      <div class="hits" id="quick-hits"></div>
    </div>
    <div id="workbench">
      <div class="banner" id="banner"></div>
      <div class="menubar">
        <span class="title">${options.label}</span>
        <button data-act="quick">File</button>
        <button data-act="undo">Edit</button>
        <button data-act="find">Selection</button>
        <button data-act="sidebar">View</button>
        <button data-act="goto">Go</button>
        <a href="${options.otherHref}" style="margin-left:auto;color:#9cdcfe;text-decoration:none">${options.otherLabel}</a>
      </div>
      <div class="body">
        <div class="activity">
          <button class="active" data-panel="files" title="Explorer">☰</button>
          <button data-panel="search" title="Search">⌕</button>
          <button data-panel="problems" title="Problems">⚠</button>
        </div>
        <div class="sidebar" id="sidebar">
          <h2 id="side-title">Explorer</h2>
          <div class="file-list" id="side-body"></div>
        </div>
        <div class="main">
          <div class="tabs" id="tabs"></div>
          <div class="editor-wrap" id="editor-wrap"><div id="editor"></div></div>
          <div class="problems hidden" id="problems"></div>
        </div>
      </div>
      <div class="status">
        <span id="status-left">${WORKSPACE}</span>
        <span id="status-right">Ln 1, Col 1</span>
      </div>
    </div>
  `;

  document.getElementById("banner").textContent = options.banner;

  if (hasTs) {
    const ts = monaco.languages.typescript;
    ts.typescriptDefaults.setCompilerOptions({
      target: ts.ScriptTarget.ES2020,
      module: ts.ModuleKind.ESNext,
      moduleResolution: ts.ModuleResolutionKind.NodeJs,
      allowNonTsExtensions: true,
      noEmit: true,
      strict: true,
    });
    ts.javascriptDefaults.setCompilerOptions({
      allowNonTsExtensions: true,
      noEmit: true,
      checkJs: true,
    });
  }

  const models = new Map();
  for (const file of FILES) {
    const uri = fileUri(monaco, file.path);
    const existing = monaco.editor.getModel?.(uri);
    const model = existing ?? monaco.editor.createModel(file.value, file.language, uri);
    models.set(file.path, { file, model, dirty: false });
  }

  const host = document.getElementById("editor");
  const wrap = document.getElementById("editor-wrap");
  const first = models.get("src/app.ts");
  const editor = monaco.editor.create(host, {
    model: first.model,
    theme: "vs-dark",
    automaticLayout: false,
    minimap: { enabled: true },
    fontSize: 14,
    lineNumbers: "on",
    scrollBeyondLastLine: false,
    padding: { top: 4 },
    tabSize: 2,
    wordWrap: "off",
    contextmenu: true,
  });

  function layout() {
    const width = Math.max(1, wrap.clientWidth);
    const height = Math.max(1, wrap.clientHeight);
    editor.layout({ width, height });
  }
  new ResizeObserver(layout).observe(wrap);
  requestAnimationFrame(layout);

  const openTabs = ["src/app.ts", "src/main.ts", "README.md"];
  let activePath = "src/app.ts";
  let sidePanel = "files";

  if (!hasTs) {
    monaco.editor.setModelMarkers(first.model, "demo", [
      {
        startLineNumber: 9,
        startColumn: 14,
        endLineNumber: 9,
        endColumn: 20,
        message: "Type 'string' is not assignable to type 'number' (sample marker; Lil has no tsc worker).",
        severity: monaco.MarkerSeverity?.Error ?? 8,
      },
    ]);
    const keywords = ["const", "export", "function", "import", "return", "string", "number"];
    for (const lang of ["typescript", "javascript"]) {
      monaco.languages.registerCompletionItemProvider(lang, {
        provideCompletionItems(model, position) {
          const word = model.getWordAtPosition?.(position);
          const start = word?.startColumn ?? position.column;
          const end = word?.endColumn ?? position.column;
          const range = {
            startLineNumber: position.lineNumber,
            startColumn: start,
            endLineNumber: position.lineNumber,
            endColumn: end,
          };
          return {
            suggestions: keywords.map((name) => ({
              label: name,
              kind: monaco.languages.CompletionItemKind?.Keyword ?? 17,
              insertText: name,
              range,
            })),
          };
        },
      });
    }
  }

  function openFile(path, pos) {
    const entry = models.get(path);
    if (!entry) {
      return;
    }
    if (!openTabs.includes(path)) {
      openTabs.push(path);
    }
    activePath = path;
    editor.setModel(entry.model);
    if (pos) {
      editor.setPosition(pos);
      editor.revealLine?.(pos.lineNumber);
    }
    editor.focus();
    render();
    layout();
  }

  function renderFiles(filter = "") {
    const q = filter.trim().toLowerCase();
    const body = document.getElementById("side-body");
    if (sidePanel === "search") {
      body.innerHTML = "";
      const input = document.createElement("input");
      input.placeholder = "Search workspace";
      input.style.cssText = "width:calc(100% - 24px);margin:4px 12px 8px;background:#3c3c3c;border:none;color:#fff;padding:6px 8px";
      body.appendChild(input);
      const hits = document.createElement("div");
      body.appendChild(hits);
      input.addEventListener("input", () => {
        const query = input.value.trim();
        hits.innerHTML = "";
        if (query.length < 2) {
          return;
        }
        for (const [path, entry] of models) {
          const found = entry.model.findMatches?.(query, true, false, false, null, true) ?? [];
          for (const match of found.slice(0, 20)) {
            const range = match.range ?? match;
            const btn = document.createElement("button");
            btn.textContent = `${path}:${range.startLineNumber}`;
            btn.addEventListener("click", () => openFile(path, { lineNumber: range.startLineNumber, column: range.startColumn ?? 1 }));
            hits.appendChild(btn);
          }
        }
      });
      return;
    }
    if (sidePanel === "problems") {
      body.innerHTML = "";
      for (const marker of markerRows(monaco)) {
        const path = [...models.entries()].find(([, e]) => {
          const uri = e.model.uri?.toString?.() ?? "";
          return uri.endsWith(e.file.path) || marker.resource.includes(e.file.path);
        })?.[0];
        const btn = document.createElement("button");
        btn.textContent = `${severityLabel(marker.severity)}  ${path ?? marker.resource}:${marker.startLineNumber}  ${marker.message}`;
        btn.addEventListener("click", () => {
          if (path) {
            openFile(path, { lineNumber: marker.startLineNumber, column: marker.startColumn });
          }
        });
        body.appendChild(btn);
      }
      if (!body.childElementCount) {
        body.textContent = "No problems.";
      }
      return;
    }
    body.innerHTML = "";
    for (const file of FILES) {
      if (q && !file.path.toLowerCase().includes(q)) {
        continue;
      }
      const btn = document.createElement("button");
      btn.textContent = file.path;
      btn.className = file.path === activePath ? "active" : "";
      btn.addEventListener("click", () => openFile(file.path));
      body.appendChild(btn);
    }
  }

  function renderTabs() {
    const tabs = document.getElementById("tabs");
    tabs.innerHTML = "";
    for (const path of openTabs) {
      const entry = models.get(path);
      const btn = document.createElement("button");
      btn.className = path === activePath ? "active" : "";
      btn.textContent = (entry?.dirty ? "● " : "") + path.split("/").pop();
      btn.addEventListener("click", () => openFile(path));
      btn.addEventListener("auxclick", (ev) => {
        if (ev.button === 1) {
          ev.preventDefault();
          closeTab(path);
        }
      });
      tabs.appendChild(btn);
    }
  }

  function closeTab(path) {
    const i = openTabs.indexOf(path);
    if (i >= 0) {
      openTabs.splice(i, 1);
    }
    if (!openTabs.length) {
      openTabs.push("README.md");
    }
    if (activePath === path) {
      openFile(openTabs[Math.max(0, i - 1)] ?? openTabs[0]);
    } else {
      render();
    }
  }

  function renderProblems() {
    const panel = document.getElementById("problems");
    const rows = markerRows(monaco);
    panel.textContent = rows.length
      ? rows.map((m) => `${severityLabel(m.severity).padEnd(7)} ${m.resource}:${m.startLineNumber}  ${m.message}`).join("\n")
      : "No problems detected.";
  }

  function renderStatus() {
    const pos = readPosition(editor);
    const lang = editor.getModel?.()?.getLanguageId?.() ?? "";
    document.getElementById("status-left").textContent = `${WORKSPACE}  ${activePath}`;
    document.getElementById("status-right").textContent = `Ln ${pos.lineNumber}, Col ${pos.column}   ${lang}   UTF-8`;
  }

  function render() {
    document.getElementById("side-title").textContent =
      sidePanel === "search" ? "Search" : sidePanel === "problems" ? "Problems" : "Explorer";
    renderFiles();
    renderTabs();
    renderProblems();
    renderStatus();
  }

  function setPanel(name) {
    sidePanel = name;
    for (const btn of document.querySelectorAll(".activity button")) {
      btn.classList.toggle("active", btn.getAttribute("data-panel") === name);
    }
    if (name === "problems") {
      document.getElementById("problems").classList.remove("hidden");
    }
    render();
  }

  function toggleQuick(open) {
    const box = document.getElementById("quick-open");
    const input = document.getElementById("quick-input");
    box.classList.toggle("open", open);
    if (open) {
      input.value = "";
      fillQuick("");
      input.focus();
    }
  }

  function fillQuick(query) {
    const hits = document.getElementById("quick-hits");
    const q = query.trim().toLowerCase();
    hits.innerHTML = "";
    for (const file of FILES) {
      if (q && !file.path.toLowerCase().includes(q)) {
        continue;
      }
      const btn = document.createElement("button");
      btn.textContent = file.path;
      btn.addEventListener("click", () => {
        toggleQuick(false);
        openFile(file.path);
      });
      hits.appendChild(btn);
    }
    hits.firstElementChild?.classList.add("active");
  }

  document.getElementById("quick-input").addEventListener("input", (ev) => fillQuick(ev.target.value));
  document.getElementById("quick-input").addEventListener("keydown", (ev) => {
    if (ev.key === "Escape") {
      toggleQuick(false);
      editor.focus();
    }
    if (ev.key === "Enter") {
      const firstHit = document.querySelector("#quick-hits button");
      firstHit?.click();
    }
  });

  document.querySelector(".menubar").addEventListener("click", (ev) => {
    const act = ev.target?.getAttribute?.("data-act");
    if (act === "quick") toggleQuick(true);
    if (act === "undo") editor.trigger("menu", "undo", null);
    if (act === "find") editor.trigger("keyboard", "actions.find", null);
    if (act === "sidebar") document.getElementById("sidebar").classList.toggle("hidden");
    if (act === "goto") editor.trigger("keyboard", "editor.action.gotoLine", null);
  });

  document.querySelector(".activity").addEventListener("click", (ev) => {
    const panel = ev.target?.getAttribute?.("data-panel");
    if (panel) setPanel(panel);
  });

  window.addEventListener("keydown", (ev) => {
    const cmd = ev.metaKey || ev.ctrlKey;
    if (cmd && ev.key.toLowerCase() === "p") {
      ev.preventDefault();
      toggleQuick(true);
    }
    if (cmd && ev.key.toLowerCase() === "b") {
      ev.preventDefault();
      document.getElementById("sidebar").classList.toggle("hidden");
      layout();
    }
    if (cmd && ev.key.toLowerCase() === "j") {
      ev.preventDefault();
      document.getElementById("problems").classList.toggle("hidden");
      layout();
    }
  });

  editor.onDidChangeCursorPosition?.(() => renderStatus());
  editor.onDidChangeModelContent?.(() => {
    const entry = models.get(activePath);
    if (entry) {
      entry.dirty = true;
      renderTabs();
    }
    renderStatus();
  });
  monaco.editor.onDidChangeMarkers?.(() => {
    renderProblems();
    if (sidePanel === "problems") renderFiles();
  });

  render();
  openFile("src/app.ts");
  return editor;
}
