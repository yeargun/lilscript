import { FILES, WORKSPACE } from "./samples.js";
import { EDITOR_COMMANDS, MENU_GROUPS } from "./editor-actions.js";

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
  const tsApi = monaco.typescript ?? monaco.languages?.typescript;
  root.innerHTML = `
    <div class="quick-open" id="quick-open">
      <input id="quick-input" placeholder="Go to file" autocomplete="off" />
      <div class="hits" id="quick-hits"></div>
    </div>
    <div id="workbench">
      <div class="banner" id="banner"></div>
      <div class="menubar">
        <span class="title">${options.label}</span>
        ${MENU_GROUPS.map(([id, label]) => `
          <div class="menu" data-menu="${id}">
            <button type="button">${label}</button>
            <div class="menu-drop">
              ${EDITOR_COMMANDS.filter((c) => c.group === id).map((c) => `<button type="button" data-cmd="${c.id}">${c.label}</button>`).join("")}
            </div>
          </div>
        `).join("")}
        <a href="${options.otherHref}" style="margin-left:auto;color:#9cdcfe;text-decoration:none">${options.otherLabel}</a>
      </div>
      <div class="body">
        <div class="activity">
          <button class="active" data-panel="files" title="Explorer">☰</button>
          <button data-panel="search" title="Search">⌕</button>
          <button data-panel="outline" title="Outline">{}</button>
          <button data-panel="problems" title="Problems">⚠</button>
        </div>
        <div class="sidebar" id="sidebar">
          <h2 id="side-title">Explorer</h2>
          <div class="file-list" id="side-body"></div>
        </div>
        <div class="main">
          <div class="tabs" id="tabs"></div>
          <div class="crumbs" id="crumbs"></div>
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
  const sizeSide = options.otherHref.includes("/js") ? "lil" : "js";
  fetch("/apps/monaco/sizes.json")
    .then((res) => (res.ok ? res.json() : null))
    .then((doc) => {
      if (!doc?.production) return;
      const ide = doc.production[sizeSide].ide;
      const workers = doc.production[sizeSide].workers ?? doc.production.workers;
      const el = document.getElementById("banner");
      const workerPart = workers?.brotli
        ? ` · workers ${workers.brotli.toLocaleString("en-US")}`
        : "";
      el.textContent =
        options.banner +
        ` ide.js Brotli ${ide.brotli.toLocaleString("en-US")}${workerPart} · sizes on the landing page.`;
    })
    .catch(() => {});

  if (tsApi?.typescriptDefaults) {
    tsApi.typescriptDefaults.setEagerModelSync?.(true);
    tsApi.javascriptDefaults.setEagerModelSync?.(true);
    tsApi.typescriptDefaults.setCompilerOptions({
      target: tsApi.ScriptTarget?.ES2020 ?? 7,
      module: tsApi.ModuleKind?.CommonJS ?? 1,
      moduleResolution: tsApi.ModuleResolutionKind?.NodeJs ?? 2,
      allowNonTsExtensions: true,
      allowImportingTsExtensions: true,
      allowJs: true,
      noEmit: true,
      strict: true,
    });
    tsApi.javascriptDefaults.setCompilerOptions({
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
    if (tsApi?.typescriptDefaults && (file.language === "typescript" || file.language === "javascript")) {
      const workerName = "/" + WORKSPACE + "/" + file.path;
      tsApi.typescriptDefaults.addExtraLib(file.value, workerName);
      tsApi.typescriptDefaults.addExtraLib(file.value, String(uri.toString?.() ?? uri));
      model.onDidChangeContent?.(() => {
        const text = model.getValue();
        tsApi.typescriptDefaults.addExtraLib(text, workerName);
        tsApi.typescriptDefaults.addExtraLib(text, String(uri.toString?.() ?? uri));
      });
    }
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
  let quickMode = "files";

  if (!tsApi?.typescriptDefaults && !options.languageFeatures) {
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
    if (sidePanel === "outline") {
      body.innerHTML = "";
      const rows = window.__lilChrome?.symbols?.() ?? [];
      for (const row of rows) {
        const btn = document.createElement("button");
        btn.textContent = `${"  ".repeat(row.depth || 0)}${row.name}`;
        btn.addEventListener("click", () => {
          editor.setPosition({ lineNumber: row.line, column: row.column });
          editor.revealLine?.(row.line);
          editor.focus();
        });
        body.appendChild(btn);
      }
      if (!body.childElementCount) {
        body.textContent = "No symbols in this file.";
      }
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
    const rows = markerRows(monaco);
    const errors = rows.filter((m) => m.severity >= 8).length;
    const warns = rows.filter((m) => m.severity >= 4 && m.severity < 8).length;
    document.getElementById("status-left").textContent = `${WORKSPACE}  ${activePath}`;
    document.getElementById("status-right").textContent =
      `${errors} errors  ${warns} warnings   Ln ${pos.lineNumber}, Col ${pos.column}   ${lang}   UTF-8`;
  }

  function renderCrumbs() {
    const crumbs = document.getElementById("crumbs");
    if (!crumbs) return;
    const rows = window.__lilChrome?.symbols?.() ?? [];
    const pos = readPosition(editor);
    let best = null;
    for (const row of rows) {
      if (row.line <= pos.lineNumber) best = row;
    }
    crumbs.textContent = best ? `${activePath} › ${best.name}` : activePath;
  }

  function render() {
    document.getElementById("side-title").textContent =
      sidePanel === "search"
        ? "Search"
        : sidePanel === "problems"
          ? "Problems"
          : sidePanel === "outline"
            ? "Outline"
            : "Explorer";
    renderFiles();
    renderTabs();
    renderProblems();
    renderStatus();
    renderCrumbs();
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

  function runEditorAction(id) {
    if (id === "workbench.action.quickOpen") return runWorkbench("quick");
    if (id === "workbench.action.showCommands") return runWorkbench("commands");
    if (id === "workbench.action.toggleSidebar") return runWorkbench("sidebar");
    if (id === "workbench.action.togglePanel") return runWorkbench("problems");
    if (id === "workbench.action.closeActiveEditor") return closeTab(activePath);
    if (window.__lilChrome?.runAction) return window.__lilChrome.runAction(id);
    editor.trigger("keyboard", id, null);
  }

  function runWorkbench(id) {
    if (id === "quick") return toggleQuick(true, "files");
    if (id === "commands") return toggleQuick(true, "commands");
    if (id === "symbols") {
      if (window.__lilChrome?.showSymbols) return window.__lilChrome.showSymbols();
      return editor.trigger("keyboard", "editor.action.quickOutline", null);
    }
    if (id === "find") return editor.trigger("keyboard", "actions.find", null);
    if (id === "replace") return editor.trigger("keyboard", "editor.action.startFindReplaceAction", null);
    if (id === "undo") return editor.trigger("menu", "undo", null);
    if (id === "redo") return editor.trigger("menu", "redo", null);
    if (id === "comment") return editor.trigger("keyboard", "editor.action.commentLine", null);
    if (id === "format") {
      if (window.__lilChrome?.formatDocument) return window.__lilChrome.formatDocument();
      return editor.trigger("keyboard", "editor.action.formatDocument", null);
    }
    if (id === "rename") {
      if (window.__lilChrome?.showRename) return window.__lilChrome.showRename();
      return editor.trigger("keyboard", "editor.action.rename", null);
    }
    if (id === "def") {
      if (window.__lilChrome?.goToDef) return window.__lilChrome.goToDef();
      return gotoDefinition();
    }
    if (id === "refs") {
      if (window.__lilChrome?.showReferences) return window.__lilChrome.showReferences();
      return editor.trigger("keyboard", "editor.action.goToReferences", null);
    }
    if (id === "suggest") {
      if (window.__lilChrome?.showSuggest) return window.__lilChrome.showSuggest();
      return editor.trigger("keyboard", "editor.action.triggerSuggest", null);
    }
    if (id === "hover") {
      if (window.__lilChrome?.showHover) return window.__lilChrome.showHover();
      return editor.trigger("keyboard", "editor.action.showHover", null);
    }
    if (id === "goto") {
      if (window.__lilChrome?.gotoLine) return window.__lilChrome.gotoLine();
      return editor.trigger("keyboard", "editor.action.gotoLine", null);
    }
    if (id === "next-problem") {
      if (window.__lilChrome?.nextProblem) return window.__lilChrome.nextProblem(1);
      return editor.trigger("keyboard", "editor.action.marker.next", null);
    }
    if (id === "sidebar") {
      document.getElementById("sidebar").classList.toggle("hidden");
      layout();
      return;
    }
    if (id === "problems") {
      document.getElementById("problems").classList.toggle("hidden");
      layout();
    }
  }

  function commandList() {
    return EDITOR_COMMANDS.map((cmd) => [cmd.label, () => runEditorAction(cmd.id)]);
  }

  function toggleQuick(open, mode = "files") {
    quickMode = mode;
    const box = document.getElementById("quick-open");
    const input = document.getElementById("quick-input");
    box.classList.toggle("open", open);
    input.placeholder = mode === "commands" ? "Run command" : mode === "symbols" ? "Go to symbol" : "Go to file";
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
    if (quickMode === "commands") {
      for (const [label, run] of commandList()) {
        if (q && !label.toLowerCase().includes(q)) continue;
        const btn = document.createElement("button");
        btn.textContent = label;
        btn.addEventListener("click", () => {
          toggleQuick(false);
          run();
        });
        hits.appendChild(btn);
      }
    } else if (quickMode === "symbols") {
      const rows = window.__lilChrome?.symbols?.() ?? [];
      for (const row of rows) {
        if (q && !String(row.name).toLowerCase().includes(q)) continue;
        const btn = document.createElement("button");
        btn.textContent = `${row.name}  ${row.kind ?? ""}`;
        btn.addEventListener("click", () => {
          toggleQuick(false);
          editor.setPosition({ lineNumber: row.line, column: row.column });
          editor.revealLine?.(row.line);
          editor.focus();
        });
        hits.appendChild(btn);
      }
    } else {
      for (const file of FILES) {
        if (q && !file.path.toLowerCase().includes(q)) continue;
        const btn = document.createElement("button");
        btn.textContent = file.path;
        btn.addEventListener("click", () => {
          toggleQuick(false);
          openFile(file.path);
        });
        hits.appendChild(btn);
      }
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

  function pathFromWorkerFile(fileName) {
    const text = String(fileName ?? "");
    return [...models.keys()].find((path) => text.endsWith(path) || text.endsWith("/" + path)) ?? null;
  }

  async function gotoDefinition() {
    const model = editor.getModel();
    const pos = readPosition(editor);
    if (!model || !pos) {
      return;
    }
    const getWorker = tsApi?.getTypeScriptWorker;
    if (getWorker) {
      const worker = await getWorker();
      const client = await worker(model.uri);
      const defs = await client.getDefinitionAtPosition(model.uri.toString(), model.getOffsetAt(pos));
      const hit = defs?.[0];
      if (hit) {
        const path = pathFromWorkerFile(hit.fileName);
        const target = (path && models.get(path)?.model) || monaco.editor.getModel(monaco.Uri.parse(hit.fileName));
        const spanStart = hit.textSpan?.start ?? 0;
        const nextPos = target?.getPositionAt?.(spanStart) ?? { lineNumber: 1, column: 1 };
        if (path) {
          openFile(path, nextPos);
        } else if (target) {
          editor.setModel(target);
          editor.setPosition(nextPos);
          editor.focus();
        }
        return;
      }
    }
    const word = model.getWordAtPosition?.(pos)?.word;
    if (word) {
      const patterns = [
        new RegExp("export\\s+(?:async\\s+)?function\\s+" + word + "\\b"),
        new RegExp("(?:export\\s+)?function\\s+" + word + "\\b"),
        new RegExp("export\\s+const\\s+" + word + "\\b"),
      ];
      for (const [path, entry] of models) {
        const text = entry.model.getValue();
        for (const re of patterns) {
          const hit = re.exec(text);
          if (hit) {
            const nextPos = entry.model.getPositionAt?.(hit.index) ?? { lineNumber: 1, column: 1 };
            openFile(path, nextPos);
            return;
          }
        }
      }
    }
    editor.trigger("keyboard", "editor.action.revealDefinition", null);
  }

  window.__ideGotoDef = gotoDefinition;

  document.querySelector(".menubar").addEventListener("click", (ev) => {
    const cmd = ev.target?.getAttribute?.("data-cmd");
    if (cmd) {
      document.querySelectorAll(".menu.open").forEach((el) => el.classList.remove("open"));
      runEditorAction(cmd);
      return;
    }
    const menu = ev.target?.closest?.(".menu");
    if (menu) {
      const open = menu.classList.contains("open");
      document.querySelectorAll(".menu.open").forEach((el) => el.classList.remove("open"));
      if (!open) menu.classList.add("open");
    }
  });
  document.addEventListener("mousedown", (ev) => {
    if (!ev.target?.closest?.(".menubar")) {
      document.querySelectorAll(".menu.open").forEach((el) => el.classList.remove("open"));
    }
  });

  document.querySelector(".activity").addEventListener("click", (ev) => {
    const panel = ev.target?.getAttribute?.("data-panel");
    if (panel) setPanel(panel);
  });

  window.addEventListener("keydown", (ev) => {
    const cmd = ev.metaKey || ev.ctrlKey;
    if (cmd && ev.key.toLowerCase() === "p") {
      ev.preventDefault();
      toggleQuick(true, ev.shiftKey ? "commands" : "files");
    }
    if (ev.key === "F1") {
      ev.preventDefault();
      toggleQuick(true, "commands");
    }
    if (cmd && ev.shiftKey && ev.key.toLowerCase() === "o") {
      ev.preventDefault();
      toggleQuick(true, "symbols");
    }
    if (ev.key === "F2") {
      ev.preventDefault();
      runWorkbench("rename");
    }
    if (ev.key === "F12" && ev.shiftKey) {
      ev.preventDefault();
      ev.stopPropagation();
      runWorkbench("refs");
    }
    if (ev.altKey && ev.shiftKey && ev.key.toLowerCase() === "f") {
      ev.preventDefault();
      runWorkbench("format");
    }
    if (ev.key === "F8") {
      ev.preventDefault();
      runWorkbench("next-problem");
    }
    if (cmd && ev.key.toLowerCase() === "b") {
      ev.preventDefault();
      document.getElementById("sidebar").classList.toggle("hidden");
      layout();
    }
    if (cmd && ev.key.toLowerCase() === "f") {
      ev.preventDefault();
      editor.trigger("keyboard", ev.shiftKey ? "editor.action.startFindReplaceAction" : "actions.find", null);
    }
    if (cmd && ev.key.toLowerCase() === "h") {
      ev.preventDefault();
      editor.trigger("keyboard", "editor.action.startFindReplaceAction", null);
    }
    if (cmd && ev.key === "/") {
      ev.preventDefault();
      editor.trigger("keyboard", "editor.action.commentLine", null);
    }
    if (cmd && ev.key.toLowerCase() === "g") {
      ev.preventDefault();
      editor.trigger("keyboard", "editor.action.gotoLine", null);
    }
    if (cmd && ev.key.toLowerCase() === "j") {
      ev.preventDefault();
      document.getElementById("problems").classList.toggle("hidden");
      layout();
    }
    if (cmd && ev.key === " ") {
      ev.preventDefault();
      editor.trigger("keyboard", "editor.action.triggerSuggest", null);
    }
    if (ev.key === "F12" && !ev.shiftKey) {
      ev.preventDefault();
      ev.stopPropagation();
      void gotoDefinition();
    }
  }, true);

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
