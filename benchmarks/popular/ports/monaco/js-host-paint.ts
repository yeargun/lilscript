const KEYWORDS = {
  javascript: "break case catch class const continue debugger default delete do else export extends false finally for function if import in instanceof let new null return super switch this throw true try typeof undefined var void while with yield async await of static get set constructor",
  typescript: "break case catch class const continue debugger default delete do else export extends false finally for function if import in instanceof let new null return super switch this throw true try typeof undefined var void while with yield async await of static get set constructor type interface enum implements package private protected public readonly namespace abstract as asserts keyof infer never unknown any boolean number string unique satisfies",
  python: "False None True and as assert async await break class continue def del elif else except finally for from global if import in is lambda nonlocal not or pass raise return try while with yield match case",
  json: "true false null",
}

function keywordSet(id) {
  const raw = KEYWORDS[id] ?? (id === "javascript" || id === "typescript" ? KEYWORDS.typescript : "")
  return new Set(raw.split(" ").filter(Boolean))
}

function escapeHtml(text) {
  return String(text ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
}

function tokenClass(type) {
  if (type.includes("comment")) return "mtk-comment"
  if (type.includes("string")) return "mtk-string"
  if (type.includes("regexp")) return "mtk-regexp"
  if (type.includes("number")) return "mtk-number"
  if (type.includes("keyword")) return "mtk-keyword"
  if (type.includes("type")) return "mtk-type"
  if (type.includes("function")) return "mtk-function"
  if (type.includes("tag")) return "mtk-tag"
  if (type.includes("attribute") || type.includes("attr")) return "mtk-attr"
  if (type.includes("key")) return "mtk-attr"
  return "mtk"
}

function pushToken(out, offset, type, text) {
  if (!text) return
  out.push({ offset, type, text })
}

function tokenizeCLike(line, keywords, state) {
  const out = []
  let i = 0
  let mode = state.mode
  while (i < line.length) {
    if (mode === "block") {
      const end = line.indexOf("*/", i)
      if (end < 0) {
        pushToken(out, i, "comment", line.slice(i))
        return { tokens: out, mode: "block" }
      }
      pushToken(out, i, "comment", line.slice(i, end + 2))
      i = end + 2
      mode = "root"
      continue
    }
    const rest = line.slice(i)
    const two = rest.slice(0, 2)
    if (two === "//") {
      pushToken(out, i, "comment", rest)
      break
    }
    if (two === "/*") {
      const end = rest.indexOf("*/")
      if (end < 0) {
        pushToken(out, i, "comment", rest)
        return { tokens: out, mode: "block" }
      }
      pushToken(out, i, "comment", rest.slice(0, end + 2))
      i += end + 2
      continue
    }
    if (rest[0] === '"' || rest[0] === "'" || rest[0] === "`") {
      const q = rest[0]
      let j = 1
      while (j < rest.length) {
        if (rest[j] === "\\") {
          j += 2
          continue
        }
        if (rest[j] === q) {
          j += 1
          break
        }
        j += 1
      }
      pushToken(out, i, "string", rest.slice(0, j))
      i += j
      continue
    }
    if (rest[0] === "/" && /[^/\s]/.test(rest[1] ?? "") && !/[a-zA-Z0-9_)]/.test(line[i - 1] ?? "")) {
      let j = 1
      while (j < rest.length && rest[j] !== "/") {
        if (rest[j] === "\\") j += 2
        else j += 1
      }
      if (rest[j] === "/") {
        j += 1
        while (/[gimsuy]/.test(rest[j] ?? "")) j += 1
        pushToken(out, i, "regexp", rest.slice(0, j))
        i += j
        continue
      }
    }
    const num = rest.match(/^(0x[0-9a-fA-F]+|\d+\.\d+([eE][+-]?\d+)?|\d+)/)
    if (num) {
      pushToken(out, i, "number", num[0])
      i += num[0].length
      continue
    }
    const word = rest.match(/^[A-Za-z_$][\w$]*/)
    if (word) {
      const next = line.slice(i + word[0].length).match(/^\s*\(/)
      let type = "identifier"
      if (keywords.has(word[0])) type = "keyword"
      else if (/^[A-Z]/.test(word[0])) type = "type"
      else if (next) type = "function"
      pushToken(out, i, type, word[0])
      i += word[0].length
      continue
    }
    pushToken(out, i, "mtk", rest[0])
    i += 1
  }
  return { tokens: out, mode }
}

function tokenizeJson(line) {
  const out = []
  let i = 0
  while (i < line.length) {
    const rest = line.slice(i)
    if (rest[0] === '"') {
      let j = 1
      while (j < rest.length && rest[j] !== '"') {
        if (rest[j] === "\\") j += 2
        else j += 1
      }
      j = Math.min(rest.length, j + 1)
      const after = line.slice(i + j).match(/^\s*:/)
      pushToken(out, i, after ? "key" : "string", rest.slice(0, j))
      i += j
      continue
    }
    const num = rest.match(/^-?\d+(\.\d+)?([eE][+-]?\d+)?/)
    if (num) {
      pushToken(out, i, "number", num[0])
      i += num[0].length
      continue
    }
    const word = rest.match(/^(true|false|null)/)
    if (word) {
      pushToken(out, i, "keyword", word[0])
      i += word[0].length
      continue
    }
    pushToken(out, i, "mtk", rest[0])
    i += 1
  }
  return { tokens: out, mode: "root" }
}

function tokenizeHtml(line, state) {
  const out = []
  let i = 0
  let mode = state.mode
  while (i < line.length) {
    const rest = line.slice(i)
    if (mode === "comment") {
      const end = rest.indexOf("-->")
      if (end < 0) {
        pushToken(out, i, "comment", rest)
        return { tokens: out, mode: "comment" }
      }
      pushToken(out, i, "comment", rest.slice(0, end + 3))
      i += end + 3
      mode = "root"
      continue
    }
    if (rest.startsWith("<!--")) {
      const end = rest.indexOf("-->")
      if (end < 0) {
        pushToken(out, i, "comment", rest)
        return { tokens: out, mode: "comment" }
      }
      pushToken(out, i, "comment", rest.slice(0, end + 3))
      i += end + 3
      continue
    }
    if (rest[0] === "<") {
      const m = rest.match(/^<\/?[\w:-]+/)
      if (m) {
        pushToken(out, i, "tag", m[0])
        i += m[0].length
        continue
      }
    }
    const attr = rest.match(/^[\w:-]+=/)
    if (attr) {
      pushToken(out, i, "attr", attr[0].slice(0, -1))
      pushToken(out, i + attr[0].length - 1, "mtk", "=")
      i += attr[0].length
      continue
    }
    if (rest[0] === '"' || rest[0] === "'") {
      const q = rest[0]
      let j = 1
      while (j < rest.length && rest[j] !== q) j += 1
      j = Math.min(rest.length, j + 1)
      pushToken(out, i, "string", rest.slice(0, j))
      i += j
      continue
    }
    pushToken(out, i, "mtk", rest[0])
    i += 1
  }
  return { tokens: out, mode }
}

function tokenizeCss(line, state) {
  const out = []
  let i = 0
  let mode = state.mode
  while (i < line.length) {
    const rest = line.slice(i)
    if (mode === "block") {
      const end = rest.indexOf("*/")
      if (end < 0) {
        pushToken(out, i, "comment", rest)
        return { tokens: out, mode: "block" }
      }
      pushToken(out, i, "comment", rest.slice(0, end + 2))
      i += end + 2
      mode = "root"
      continue
    }
    if (rest.startsWith("/*")) {
      const end = rest.indexOf("*/")
      if (end < 0) {
        pushToken(out, i, "comment", rest)
        return { tokens: out, mode: "block" }
      }
      pushToken(out, i, "comment", rest.slice(0, end + 2))
      i += end + 2
      continue
    }
    if (rest[0] === '"' || rest[0] === "'") {
      const q = rest[0]
      let j = 1
      while (j < rest.length && rest[j] !== q) j += 1
      j = Math.min(rest.length, j + 1)
      pushToken(out, i, "string", rest.slice(0, j))
      i += j
      continue
    }
    const num = rest.match(/^-?[\d.]+(px|em|rem|%|vh|vw|s|ms)?/)
    if (num) {
      pushToken(out, i, "number", num[0])
      i += num[0].length
      continue
    }
    const word = rest.match(/^[A-Za-z_-][\w-]*/)
    if (word) {
      const type = rest.slice(word[0].length).match(/^\s*:/) ? "attr" : "keyword"
      pushToken(out, i, type, word[0])
      i += word[0].length
      continue
    }
    pushToken(out, i, "mtk", rest[0])
    i += 1
  }
  return { tokens: out, mode }
}

function tokenizeMarkdown(line) {
  const out = []
  if (/^#{1,6}\s/.test(line)) {
    pushToken(out, 0, "keyword", line)
    return { tokens: out, mode: "root" }
  }
  if (/^```/.test(line)) {
    pushToken(out, 0, "string", line)
    return { tokens: out, mode: "root" }
  }
  let i = 0
  while (i < line.length) {
    const rest = line.slice(i)
    const code = rest.match(/^`[^`]+`/)
    if (code) {
      pushToken(out, i, "string", code[0])
      i += code[0].length
      continue
    }
    pushToken(out, i, "mtk", rest[0])
    i += 1
  }
  return { tokens: out, mode: "root" }
}

function tokenizePython(line, keywords, state) {
  if (state.mode === "block") {
    const end = line.indexOf('"""')
    if (end < 0) return { tokens: [{ offset: 0, type: "comment", text: line }], mode: "block" }
    return tokenizeCLike(line, keywords, { mode: "root" })
  }
  if (line.trimStart().startsWith("#")) {
    return { tokens: [{ offset: 0, type: "comment", text: line }], mode: "root" }
  }
  return tokenizeCLike(line, keywords, state)
}

function tokenizeLine(languageId, line, state) {
  if (languageId === "json") return tokenizeJson(line)
  if (languageId === "html") return tokenizeHtml(line, state)
  if (languageId === "css") return tokenizeCss(line, state)
  if (languageId === "markdown") return tokenizeMarkdown(line)
  if (languageId === "python") return tokenizePython(line, keywordSet("python"), state)
  if (languageId === "javascript" || languageId === "typescript") {
    return tokenizeCLike(line, keywordSet(languageId), state)
  }
  return { tokens: [{ offset: 0, type: "mtk", text: line }], mode: "root" }
}

function tokensToHtml(tokens) {
  let html = ""
  for (const tok of tokens) {
    html += `<span class="${tokenClass(tok.type)}">${escapeHtml(tok.text)}</span>`
  }
  return html || "<span>&nbsp;</span>"
}

function expandTabs(line, tabSize) {
  const size = tabSize > 0 ? tabSize : 4
  let out = ""
  let col = 0
  for (const ch of line) {
    if (ch === "\t") {
      const n = size - (col % size)
      out += " ".repeat(n)
      col += n
    } else {
      out += ch
      col += 1
    }
  }
  return out
}

function measureCharWidth(el, fontSize) {
  const canvas = document.createElement("canvas")
  const ctx = canvas.getContext("2d")
  if (!ctx) return 8
  const win = el?.ownerDocument?.defaultView
  const cs = win && typeof win.getComputedStyle === "function" ? win.getComputedStyle(el) : null
  ctx.font = cs && cs.font ? cs.font : `${fontSize || 14}px ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace`
  const w = ctx.measureText("m").width
  return w > 1 ? w : 8
}

function findMatches(lines, query) {
  if (!query) return []
  const hits = []
  for (let i = 0; i < lines.length; i++) {
    let from = 0
    while (from <= lines[i].length) {
      const at = lines[i].indexOf(query, from)
      if (at < 0) break
      hits.push({ line: i + 1, start: at + 1, end: at + query.length + 1 })
      from = at + Math.max(1, query.length)
    }
  }
  return hits
}

function matchBracket(lines, line, col) {
  const pairs = { "(": ")", "[": "]", "{": "}" }
  const close = { ")": "(", "]": "[", "}": "{" }
  const text = lines.join("\n")
  let offset = 0
  for (let i = 0; i < line - 1; i++) offset += lines[i].length + 1
  offset += col - 1
  if (offset < 0 || offset >= text.length) return null
  const ch = text[offset]
  if (pairs[ch]) {
    let depth = 1
    for (let i = offset + 1; i < text.length; i++) {
      if (text[i] === ch) depth++
      else if (text[i] === pairs[ch]) {
        depth--
        if (depth === 0) return i
      }
    }
  }
  if (close[ch]) {
    let depth = 1
    for (let i = offset - 1; i >= 0; i--) {
      if (text[i] === ch) depth++
      else if (text[i] === close[ch]) {
        depth--
        if (depth === 0) return i
      }
    }
  }
  return null
}

function offsetToPos(lines, offset) {
  let left = offset
  for (let i = 0; i < lines.length; i++) {
    if (left <= lines[i].length) return { line: i + 1, column: left + 1 }
    left -= lines[i].length + 1
  }
  return { line: lines.length, column: (lines[lines.length - 1] ?? "").length + 1 }
}

function foldHidden(folds, line) {
  for (const f of folds) {
    if (f.collapsed && line > f.start && line <= f.end) return true
  }
  return false
}

function hiddenBefore(folds, line) {
  let n = 0
  for (const f of folds) {
    if (f.collapsed && f.end < line) n += f.end - f.start
  }
  return n
}

function indentCols(line) {
  const m = String(line ?? "").match(/^[\t ]*/)
  return (m ? m[0] : "").replace(/\t/g, "  ").length
}

function foldStarts(folds, line) {
  return folds.find((f) => f.start === line) ?? null
}

function bindMarginOnce(margin) {
  if (!margin || margin.getAttribute("data-fold-bound") === "1") return
  margin.setAttribute("data-fold-bound", "1")
  margin.addEventListener("mousedown", (ev) => {
    const hit = ev.target?.closest?.("[data-fold]")
    if (!hit) return
    ev.preventDefault()
    ev.stopPropagation()
    const line = Number(hit.getAttribute("data-fold"))
    globalThis.__lilChrome?.toggleFoldAt?.(line)
  })
}

function bindMinimapOnce(minimap) {
  if (!minimap || minimap.getAttribute("data-nav-bound") === "1") return
  minimap.setAttribute("data-nav-bound", "1")
  minimap.addEventListener("mousedown", (ev) => {
    const ed = globalThis.monaco?.editor?.getEditors?.()?.[0]
    const model = ed?.getModel?.()
    if (!ed || !model) return
    const rect = minimap.getBoundingClientRect()
    const y = ev.clientY - rect.top
    const line = Math.max(1, Math.min(model.getLineCount(), Math.floor((y / Math.max(1, rect.height)) * model.getLineCount()) + 1))
    ed.setPosition({ lineNumber: line, column: 1 })
    ed.revealLine?.(line)
    ed.focus?.()
  })
}

let cssInjected = false
function injectCss() {
  if (cssInjected || typeof document === "undefined") return
  cssInjected = true
  const style = document.createElement("style")
  style.textContent = `
@keyframes lil-cursor-blink{0%,49%{opacity:1}50%,100%{opacity:0}}
.monaco-editor .view-lines{position:relative;font-variant-ligatures:none}
.monaco-editor .view-line{position:absolute;left:0;right:0;white-space:pre;padding-left:0}
.monaco-editor .line-number{position:absolute;left:0;right:8px;color:#858585;text-align:right;user-select:none}
.monaco-editor .current-line{position:absolute;left:0;right:0;background:rgba(255,255,255,.04);pointer-events:none}
.monaco-editor .selected-text{position:absolute;background:rgba(38,79,120,.45);pointer-events:none}
.monaco-editor .find-match{position:absolute;background:rgba(234,192,98,.28);pointer-events:none}
.monaco-editor .bracket-match{position:absolute;border-bottom:1px solid #888;pointer-events:none}
.monaco-editor .word-highlight{position:absolute;pointer-events:none}
.monaco-editor .indent-guide{position:absolute;width:1px;background:rgba(255,255,255,.08);pointer-events:none}
.monaco-editor .folding{cursor:pointer;display:inline-block;width:12px;color:#c5c5c5}
.monaco-editor .folded-hint{opacity:.55;color:#808080}

.monaco-editor .mtk{color:#d4d4d4}
.monaco-editor .mtk-keyword{color:#569cd6}
.monaco-editor .mtk-string{color:#ce9178}
.monaco-editor .mtk-comment{color:#6a9955}
.monaco-editor .mtk-number{color:#b5cea8}
.monaco-editor .mtk-type{color:#4ec9b0}
.monaco-editor .mtk-function{color:#dcdcaa}
.monaco-editor .mtk-tag{color:#569cd6}
.monaco-editor .mtk-attr{color:#9cdcfe}
.monaco-editor .mtk-regexp{color:#d16969}
.monaco-editor .margin{position:relative;background:#1e1e1e;border-right:1px solid #2b2b2b}
.monaco-editor .find-widget input{background:#3c3c3c;border:1px solid #3c3c3c;color:#ccc;margin:2px;padding:4px 6px}
.monaco-editor .find-widget button{background:#0e639c;border:none;color:#fff;margin:2px;padding:4px 8px;cursor:pointer}
.monaco-editor textarea.inputarea{z-index:6}
`
  document.head.appendChild(style)
}

const paintState = new WeakMap()

export function hostPaintEditor(
  linesHost,
  margin,
  selectionHost,
  cursorEl,
  minimap,
  textarea,
  value,
  languageId,
  lineHeight,
  width,
  height,
  scrollTop,
  showLineNumbers,
  showMinimap,
  theme,
  selStartLine,
  selStartCol,
  posLine,
  posCol,
  findQuery,
  wordWrap,
  tabSize,
) {
  if (!linesHost) return 8
  injectCss()
  const rawLines = String(value ?? "").split("\n")
  const lines = rawLines.map((line) => expandTabs(line, tabSize | 0 || 4))
  const lh = Math.max(1, lineHeight | 0 || 19)
  const charWidth = measureCharWidth(linesHost, 14)
  const dark = theme === "vs-dark" || theme === "hc-black"
  const models = globalThis.monaco?.editor?.getModels?.() ?? []
  const current = models.find((m) => m.getValue?.() === String(value ?? ""))
  const folds = current?._paintFolds ?? []
  const visibleCount = lines.length - folds.reduce((n, f) => n + (f.collapsed ? f.end - f.start : 0), 0)
  const totalH = Math.max(lh, visibleCount * lh)
  const paintHeight = (height | 0) > 0 ? (height | 0) : Math.min(2400, Math.max(lh * 24, visibleCount * lh))
  let state = { mode: "root" }
  const vis = Math.max(12, Math.ceil(paintHeight / lh) + 8)
  const visualStart = Math.max(0, Math.floor((scrollTop | 0) / lh) - 2)
  linesHost.style.position = "relative"
  linesHost.style.height = totalH + "px"
  linesHost.style.whiteSpace = wordWrap ? "pre-wrap" : "pre"
  if (margin) {
    margin.style.position = "relative"
    margin.style.height = totalH + "px"
    margin.style.background = dark ? "#1e1e1e" : "#f3f3f3"
    bindMarginOnce(margin)
  }

  function yOf(line) {
    return (line - 1 - hiddenBefore(folds, line)) * lh
  }

  const visualLines = []
  for (let i = 0; i < lines.length; i++) {
    const line = i + 1
    if (foldHidden(folds, line)) continue
    visualLines.push(line)
  }
  const from = Math.min(visualLines.length, visualStart)
  const to = Math.min(visualLines.length, from + vis)
  for (let i = 0; i < (visualLines[from] ?? 1) - 1; i++) {
    state = tokenizeLine(languageId || "plaintext", lines[i], state)
  }
  let linesHtml = ""
  let marginHtml = ""
  for (let v = from; v < to; v++) {
    const line = visualLines[v]
    const i = line - 1
    const tok = tokenizeLine(languageId || "plaintext", lines[i], state)
    state = { mode: tok.mode }
    const top = yOf(line)
    const currentLine = line === (posLine | 0)
    const fold = foldStarts(folds, line)
    const foldedHint = fold?.collapsed ? `<span class="folded-hint"> ⋯</span>` : ""
    linesHtml += `<div class="view-line${currentLine ? " current" : ""}" data-line="${line}" style="top:${top}px;height:${lh}px">${tokensToHtml(tok.tokens)}${foldedHint}</div>`
    if (showLineNumbers) {
      const glyph = fold ? `<span class="folding" data-fold="${line}">${fold.collapsed ? "▶" : "▼"}</span>` : `<span class="folding"></span>`
      marginHtml += `<div class="line-number" data-line="${line}" style="top:${top}px;height:${lh}px;line-height:${lh}px">${glyph}${line}</div>`
    }
  }

  linesHost.innerHTML = linesHtml
  if (margin) margin.innerHTML = marginHtml

  const selHtml = []
  function paintSel(sl, sc, eline, ec) {
    if (sl > eline || (sl === eline && sc > ec)) {
      const tl = sl
      const tc = sc
      sl = eline
      sc = ec
      eline = tl
      ec = tc
    }
    if (sl === eline && sc === ec) return
    for (let line = sl; line <= eline; line++) {
      if (foldHidden(folds, line)) continue
      const startCol = line === sl ? sc : 1
      const endCol = line === eline ? ec : (lines[line - 1] ?? "").length + 1
      const left = (Math.max(1, startCol) - 1) * charWidth
      const widthPx = Math.max(charWidth * 0.4, (endCol - startCol) * charWidth)
      selHtml.push(`<div class="selected-text" style="top:${yOf(line)}px;left:${left}px;width:${widthPx}px;height:${lh}px"></div>`)
    }
  }
  paintSel(selStartLine | 0, selStartCol | 0, posLine | 0, posCol | 0)
  const extraSels = globalThis.monaco?.editor?.getEditors?.()?.[0]?.getSelections?.() ?? []
  for (let i = 1; i < extraSels.length; i++) {
    const s = extraSels[i]
    paintSel(s.startLineNumber, s.startColumn, s.endLineNumber, s.endColumn)
  }
  if (!foldHidden(folds, posLine | 0)) {
    selHtml.push(`<div class="current-line" style="top:${yOf(posLine | 0)}px;height:${lh}px"></div>`)
  }
  const tab = tabSize | 0 || 2
  for (let v = from; v < to; v++) {
    const line = visualLines[v]
    const width = indentCols(lines[line - 1] ?? "")
    for (let col = tab; col <= width; col += tab) {
      selHtml.push(`<div class="indent-guide core-guide" style="top:${yOf(line)}px;left:${col * charWidth}px;height:${lh}px"></div>`)
    }
  }
  const q = String(findQuery ?? "")
  if (q.length > 0) {
    for (const hit of findMatches(lines, q)) {
      if (foldHidden(folds, hit.line)) continue
      const left = (hit.start - 1) * charWidth
      const widthPx = Math.max(charWidth, (hit.end - hit.start) * charWidth)
      selHtml.push(`<div class="find-match" style="top:${yOf(hit.line)}px;left:${left}px;width:${widthPx}px;height:${lh}px"></div>`)
    }
  }
  const br = matchBracket(rawLines, posLine | 0, posCol | 0)
  if (br != null) {
    const other = offsetToPos(rawLines, br)
    for (const p of [
      { line: posLine | 0, column: posCol | 0 },
      other,
    ]) {
      if (foldHidden(folds, p.line)) continue
      selHtml.push(`<div class="bracket-match" style="top:${yOf(p.line)}px;left:${(p.column - 1) * charWidth}px;width:${charWidth}px;height:${lh}px"></div>`)
    }
  }
  const marks = current?._paintMarkers ?? []
  for (const m of marks) {
    const line = m.line | 1
    if (foldHidden(folds, line)) continue
    const sc = m.sc | 1
    const ec = Math.max(sc + 1, m.ec | (sc + 1))
    const color = (m.severity | 0) >= 8 ? "#f14c4c" : "#cca700"
    selHtml.push(
      `<div style="position:absolute;top:${yOf(line) + lh - 3}px;left:${(sc - 1) * charWidth}px;width:${Math.max(charWidth, (ec - sc) * charWidth)}px;height:3px;background:${color};pointer-events:none"></div>`,
    )
  }
  const highs = current?._paintHighlights ?? []
  for (const h of highs) {
    if (foldHidden(folds, h.line | 1)) continue
    const left = ((h.sc | 1) - 1) * charWidth
    const widthPx = Math.max(charWidth, ((h.ec | 1) - (h.sc | 1)) * charWidth)
    selHtml.push(
      `<div class="word-highlight" style="top:${yOf(h.line | 1)}px;left:${left}px;width:${widthPx}px;height:${lh}px;background:rgba(87,131,196,.28)"></div>`,
    )
  }
  if (selectionHost) {
    selectionHost.style.position = "absolute"
    selectionHost.style.inset = "0"
    selectionHost.style.pointerEvents = "none"
    selectionHost.innerHTML = selHtml.join("")
  }

  const caretTop = yOf(posLine | 0)
  const caretLeft = ((posCol | 0) - 1) * charWidth
  if (cursorEl) {
    cursorEl.style.top = caretTop + "px"
    cursorEl.style.left = caretLeft + "px"
    cursorEl.style.height = lh + "px"
    cursorEl.style.width = "2px"
    cursorEl.style.background = dark ? "#aeafad" : "#000"
    cursorEl.style.display = foldHidden(folds, posLine | 0) ? "none" : "block"
  }
  if (textarea) {
    textarea.style.position = "absolute"
    textarea.style.top = "0"
    textarea.style.left = "56px"
    textarea.style.width = "calc(100% - 120px)"
    textarea.style.height = "100%"
    textarea.style.opacity = "0"
    textarea.style.zIndex = "6"
    textarea.style.pointerEvents = "none"
    textarea.value = ""
  }

  if (minimap && showMinimap) {
    const w = 64
    const h = Math.max(1, height | 0)
    minimap.width = w
    minimap.height = h
    minimap.style.display = "block"
    bindMinimapOnce(minimap)
    const ctx = minimap.getContext("2d")
    if (ctx) {
      ctx.fillStyle = dark ? "#1e1e1e" : "#f3f3f3"
      ctx.fillRect(0, 0, w, h)
      ctx.fillStyle = dark ? "#5a5a5a" : "#6e6e6e"
      const row = Math.max(1, h / Math.max(1, lines.length))
      for (let i = 0; i < lines.length; i++) {
        const len = Math.min(60, lines[i].length)
        if (len > 0) ctx.fillRect(2, i * row, len, Math.max(1, row))
      }
      ctx.fillStyle = "#f14c4c"
      for (const m of marks) {
        ctx.fillRect(w - 4, ((m.line | 1) - 1) * row, 3, Math.max(2, row))
      }
      ctx.fillStyle = "rgba(255,255,255,0.08)"
      const viewY = ((scrollTop | 0) / Math.max(1, totalH)) * h
      ctx.fillRect(0, viewY, w, Math.max(8, (height / Math.max(1, totalH)) * h))
    }
  } else if (minimap && !showMinimap) {
    minimap.style.display = "none"
  }

  paintState.set(linesHost, { charWidth, lineHeight: lh, lines, visualLines, folds, scrollable: linesHost.parentElement })
  return charWidth | 0
}

export function hostCoordsToPosition(scrollable, linesHost, clientX, clientY, lineHeight, charWidth, scrollTop) {
  const state = linesHost ? paintState.get(linesHost) : null
  const lh = state?.lineHeight || lineHeight || 19
  const cw = state?.charWidth || charWidth || 8
  const el = scrollable || linesHost
  const rect = el.getBoundingClientRect()
  const y = clientY - rect.top + (el.scrollTop | scrollTop | 0)
  const x = clientX - rect.left
  let line = Math.floor(y / lh)
  const visual = state?.visualLines
  if (visual && visual.length) {
    if (line < 0) line = 0
    if (line >= visual.length) line = visual.length - 1
    line = visual[line]
  } else {
    line = line + 1
    const max = state?.lines?.length || 1
    if (line < 1) line = 1
    if (line > max) line = max
  }
  let col = Math.floor(x / cw) + 1
  const lineLen = (state?.lines?.[line - 1] ?? "").length + 1
  if (col < 1) col = 1
  if (col > lineLen) col = lineLen
  return [line, col]
}

export function eventIsComposing(event) {
  return !!(event && event.isComposing)
}

export function eventDeltaY(event) {
  return event && event.deltaY ? event.deltaY | 0 : 0
}

export function hostStyleFindWidget(findWidget, findInput, replaceInput) {
  if (!findWidget || findWidget.getAttribute("data-ready") === "1") return
  findWidget.setAttribute("data-ready", "1")
  const next = document.createElement("button")
  next.textContent = "Next"
  next.setAttribute("data-find", "next")
  const prev = document.createElement("button")
  prev.textContent = "Prev"
  prev.setAttribute("data-find", "prev")
  const repl = document.createElement("button")
  repl.textContent = "Replace"
  repl.setAttribute("data-find", "replace")
  const all = document.createElement("button")
  all.textContent = "All"
  all.setAttribute("data-find", "all")
  findWidget.appendChild(next)
  findWidget.appendChild(prev)
  findWidget.appendChild(repl)
  findWidget.appendChild(all)
}
