function compileMonarch(languageId, def) {
  const spec = def && typeof def.then !== "function" ? def : { tokenizer: { root: [] } }
  const tokenizer = spec.tokenizer ?? {}
  const keywords = spec.keywords ?? []
  const postfix = spec.tokenPostfix ?? "." + languageId
  const defaultToken = spec.defaultToken ?? "source"

  function expandInclude(state, seen) {
    const rules = tokenizer[state] ?? []
    const out = []
    for (const rule of rules) {
      if (rule && typeof rule === "object" && typeof rule.include === "string") {
        const name = rule.include.replace(/^@/, "")
        if (!seen.has(name)) {
          seen.add(name)
          out.push(...expandInclude(name, seen))
        }
        continue
      }
      out.push(rule)
    }
    return out
  }

  function tokenName(token, text) {
    if (token == null) {
      return defaultToken
    }
    if (typeof token === "string") {
      if (token === "@brackets") {
        return "delimiter.bracket" + postfix
      }
      if (token.indexOf(".") >= 0 || postfix.length === 0) {
        return token
      }
      return token + postfix
    }
    if (token.cases) {
      const word = text
      if (token.cases["@keywords"] && keywords.includes(word)) {
        return tokenName(token.cases["@keywords"], word)
      }
      if (token.cases["@default"]) {
        return tokenName(token.cases["@default"], word)
      }
      return defaultToken
    }
    if (typeof token.token === "string") {
      return tokenName(token.token, text)
    }
    return defaultToken
  }

  function nextState(rule, stack) {
    const action = Array.isArray(rule) ? rule[1] : rule
    if (!action || typeof action === "string") {
      return stack
    }
    const next = action.next ?? action.switchTo
    if (typeof next !== "string") {
      return stack
    }
    const copy = stack.slice()
    if (next === "@pop") {
      if (copy.length > 1) {
        copy.pop()
      }
      return copy
    }
    if (next.startsWith("@")) {
      copy.push(next.slice(1))
      return copy
    }
    copy[copy.length - 1] = next
    return copy
  }

  function toRegex(rule) {
    const pat = Array.isArray(rule) ? rule[0] : rule.regex
    if (pat instanceof RegExp) {
      const src = pat.source.startsWith("^") ? pat.source : "^(?:" + pat.source + ")"
      return new RegExp(src)
    }
    const src = String(pat ?? "")
    return new RegExp(src.startsWith("^") ? src : "^(?:" + src + ")")
  }

  return {
    tokenize(line) {
      const tokens = []
      let pos = 0
      let stack = ["root"]
      let guard = 0
      while (pos < line.length && guard < 10000) {
        guard++
        const state = stack[stack.length - 1] ?? "root"
        const rules = expandInclude(state, new Set())
        let matched = ""
        let matchedRule = null
        for (const rule of rules) {
          const re = toRegex(rule)
          const found = re.exec(line.slice(pos))
          if (found && found[0] != null) {
            matched = found[0]
            matchedRule = rule
            break
          }
        }
        if (!matchedRule) {
          matched = line.charAt(pos)
          tokens.push({ offset: pos, type: defaultToken })
          pos += 1
          continue
        }
        if (matched.length === 0) {
          matched = line.charAt(pos) || ""
          if (!matched) {
            break
          }
        }
        const action = Array.isArray(matchedRule) ? matchedRule[1] : matchedRule
        const type = tokenName(action, matched)
        if (type && type !== "@rematch") {
          tokens.push({ offset: pos, type })
          pos += matched.length
        }
        stack = nextState(matchedRule, stack)
      }
      return { tokens, endState: stack }
    },
  }
}

export function bindMonaco(lil) {
  if (typeof lil.bootLanguages === "function") {
    lil.bootLanguages()
  }

  const KeyMod = {
    CtrlCmd: lil.KeyModCtrl ?? 2048,
    Shift: lil.KeyModShift ?? 1024,
    Alt: lil.KeyModAlt ?? 512,
    WinCtrl: lil.KeyModWinCtrl ?? 256,
    chord(first, second) {
      return first | second
    },
  }

  const MarkerSeverity = {
    Hint: lil.MarkerSeverityHint ?? 1,
    Info: lil.MarkerSeverityInfo ?? 2,
    Warning: lil.MarkerSeverityWarning ?? 4,
    Error: lil.MarkerSeverityError ?? 8,
  }

  const CompletionItemKind = {
    Method: 0,
    Function: 1,
    Constructor: 2,
    Field: 3,
    Variable: 4,
    Class: 5,
    Struct: 6,
    Interface: 7,
    Module: 8,
    Property: 9,
    Event: 10,
    Operator: 11,
    Unit: 12,
    Value: 13,
    Constant: 14,
    Enum: 15,
    EnumMember: 16,
    Keyword: 17,
    Text: 18,
    Color: 19,
    File: 20,
    Reference: 21,
    Customcolor: 22,
    Folder: 23,
    TypeParameter: 24,
    User: 25,
    Issue: 26,
    Snippet: 27,
  }

  function Position(lineNumber, column) {
    return lil.Position ? lil.Position(lineNumber, column) : { lineNumber, column }
  }

  function asPosition(pos) {
    if (!pos) {
      return { lineNumber: 1, column: 1 }
    }
    if (Array.isArray(pos) && pos.length >= 2) {
      return { lineNumber: Number(pos[0]) || 1, column: Number(pos[1]) || 1 }
    }
    const line = Number(pos.lineNumber ?? pos.positionLineNumber ?? 1) || 1
    const col = Number(pos.column ?? pos.positionColumn ?? 1) || 1
    return { lineNumber: line, column: col }
  }

  function lilPos(pos) {
    const at = asPosition(pos)
    return lil.Position ? lil.Position(at.lineNumber, at.column) : at
  }

  function wordAtColumn(text, column) {
    const idx = Math.max(0, (column | 0) - 1)
    const re = /[A-Za-z_$][\w$]*/g
    let match
    while ((match = re.exec(String(text ?? "")))) {
      const start = match.index
      const end = start + match[0].length
      if (idx >= start && idx < end) {
        return { word: match[0], startColumn: start + 1, endColumn: end + 1 }
      }
    }
    return null
  }

  function positionFromOffset(text, offset) {
    const lines = String(text ?? "").split("\n")
    let rest = Math.max(0, offset | 0)
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i]
      if (rest <= line.length) {
        return Position(i + 1, rest + 1)
      }
      rest -= line.length + 1
    }
    const last = lines[lines.length - 1] ?? ""
    return Position(lines.length, last.length + 1)
  }

  function Range(startLineNumber, startColumn, endLineNumber, endColumn) {
    return lil.Range
      ? lil.Range(startLineNumber, startColumn, endLineNumber, endColumn)
      : { startLineNumber, startColumn, endLineNumber, endColumn }
  }

  function Selection(selectionStartLineNumber, selectionStartColumn, positionLineNumber, positionColumn) {
    return lil.Selection
      ? lil.Selection(selectionStartLineNumber, selectionStartColumn, positionLineNumber, positionColumn)
      : { selectionStartLineNumber, selectionStartColumn, positionLineNumber, positionColumn }
  }

  function unwrap(value) {
    return value && value._handle ? value._handle : value
  }

  function disposable(fn) {
    return { dispose: typeof fn === "function" ? fn : () => {} }
  }

  function eventHub() {
    const fns = []
    const event = (listener) => {
      fns.push(listener)
      return disposable(() => {
        const i = fns.indexOf(listener)
        if (i >= 0) {
          fns.splice(i, 1)
        }
      })
    }
    event._fire = (payload) => {
      for (const fn of fns.slice()) {
        fn(payload)
      }
    }
    return event
  }

  function packSels(sels) {
    const packed = []
    for (const sel of sels ?? []) {
      packed.push(
        sel.selectionStartLineNumber ?? sel.startLineNumber ?? 1,
        sel.selectionStartColumn ?? sel.startColumn ?? 1,
        sel.positionLineNumber ?? sel.endLineNumber ?? 1,
        sel.positionColumn ?? sel.endColumn ?? 1,
      )
    }
    return packed
  }

  function unpackSels(packed) {
    const out = []
    for (let i = 0; i + 3 < (packed ?? []).length; i += 4) {
      out.push(Selection(packed[i], packed[i + 1], packed[i + 2], packed[i + 3]))
    }
    return out
  }

  function escapeHtml(text) {
    return String(text ?? "")
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
  }

  const modelCreated = eventHub()
  const modelDisposed = eventHub()
  const modelLanguage = eventHub()
  const markersChanged = eventHub()
  const languageEncountered = new Map()
  const globalCommands = []
  const globalActions = []
  const keybindingRules = []
  const linkOpeners = []
  const editorOpeners = []
  const modelCache = new Map()
  const editorCache = new Map()
  let colorMap = null
  let editorSeq = 0

  class CancellationTokenSource {
    constructor() {
      this.token = {
        isCancellationRequested: false,
        onCancellationRequested: eventHub(),
      }
    }
    cancel() {
      this.token.isCancellationRequested = true
      this.token.onCancellationRequested._fire()
    }
    dispose() {}
  }

  class Emitter {
    constructor() {
      this.event = eventHub()
    }
    fire(value) {
      this.event._fire(value)
    }
    dispose() {}
  }

  function wrapModel(handle) {
    if (!handle) {
      return null
    }
    if (handle._handle && handle.getValue) {
      return handle
    }
    const cached = modelCache.get(handle)
    if (cached) {
      return cached
    }
    const uriString = typeof lil.modelUriString === "function" ? lil.modelUriString(handle) : "inmemory://model"
    const uri = {
      scheme: "inmemory",
      authority: "",
      path: uriString,
      query: "",
      fragment: "",
      fsPath: uriString,
      toString() {
        return uriString
      },
    }
    const model = {
      _handle: handle,
      uri,
      getValue() {
        return lil.modelGetValue ? lil.modelGetValue(handle) : ""
      },
      setValue(value) {
        lil.modelSetValue?.(handle, value ?? "")
      },
      getLineCount() {
        return lil.modelGetLineCount ? lil.modelGetLineCount(handle) : 1
      },
      getLineContent(line) {
        return lil.modelGetLineContent ? lil.modelGetLineContent(handle, line) : ""
      },
      getLineLength(line) {
        return (model.getLineContent(line) ?? "").length
      },
      getLanguageId() {
        return lil.modelGetLanguageId ? lil.modelGetLanguageId(handle) : "plaintext"
      },
      getVersionId() {
        return lil.modelGetVersionId ? lil.modelGetVersionId(handle) : 1
      },
      getOffsetAt(pos) {
        const line = pos?.lineNumber | 0 || 1
        const col = pos?.column | 0 || 1
        if (lil.Position && lil.modelGetOffsetAt) {
          const off = lil.modelGetOffsetAt(handle, lilPos({ lineNumber: line, column: col }))
          if (off > 0 || (line === 1 && col <= 1)) {
            return off
          }
        }
        const lines = model.getValue().split("\n")
        let offset = 0
        for (let i = 0; i < line - 1 && i < lines.length; i++) {
          offset += lines[i].length + 1
        }
        return offset + Math.max(0, col - 1)
      },
      getPositionAt(offset) {
        if (lil.modelGetPositionAt) {
          const pos = lil.modelGetPositionAt(handle, offset | 0)
          const line = Number(pos?.lineNumber ?? 0)
          const col = Number(pos?.column ?? 0)
          if (line > 1 || col > 1 || (offset | 0) === 0) {
            return asPosition(pos)
          }
        }
        return positionFromOffset(model.getValue(), offset | 0)
      },
      getValueInRange(range) {
        return lil.modelGetValueInRange ? lil.modelGetValueInRange(handle, range) : ""
      },
      getWordAtPosition(pos) {
        const at = asPosition(pos)
        if (lil.modelGetWordAtPosition) {
          const row = lil.modelGetWordAtPosition(handle, lilPos(at))
          if (row && row.length >= 3 && row[2]) {
            return { word: row[2], startColumn: Number(row[0]), endColumn: Number(row[1]) }
          }
        }
        return wordAtColumn(model.getLineContent(at.lineNumber), at.column)
      },
      getFullModelRange() {
        const last = model.getLineCount()
        const col = (model.getLineContent(last) ?? "").length + 1
        return Range(1, 1, last, col)
      },
      getEOL() {
        return "\n"
      },
      findMatches(query, _searchScope, isRegex, matchCase, wholeWord, _capture, limit) {
        if (lil.modelFindMatches) {
          return lil.modelFindMatches(handle, query ?? "", !!isRegex, matchCase !== false, !!wholeWord, limit ?? 1000)
        }
        return []
      },
      applyEdits(edits) {
        const ops = (edits ?? []).map((e) =>
          lil.editOp(
            e.range.startLineNumber,
            e.range.startColumn,
            e.range.endLineNumber,
            e.range.endColumn,
            e.text ?? "",
          ),
        )
        return lil.modelApplyEdits ? lil.modelApplyEdits(handle, ops) : []
      },
      pushEditOperations(_before, edits) {
        return model.applyEdits(edits)
      },
      deltaDecorations(oldIds, next) {
        if (!lil.modelDeltaDecorations || !lil.deco) {
          return []
        }
        const decos = (next ?? []).map((d) =>
          lil.deco(
            d.range.startLineNumber,
            d.range.startColumn,
            d.range.endLineNumber,
            d.range.endColumn,
            d.options?.inlineClassName ?? d.options?.className ?? d.options?.glyphMarginClassName ?? "",
          ),
        )
        return lil.modelDeltaDecorations(handle, oldIds ?? [], decos)
      },
      undo() {
        return lil.modelUndo ? lil.modelUndo(handle) : false
      },
      redo() {
        return lil.modelRedo ? lil.modelRedo(handle) : false
      },
      dispose() {
        lil.modelDispose?.(handle)
      },
      onDidChangeContent(listener) {
        if (typeof lil.modelOnDidChangeContent === "function") {
          const dispose = lil.modelOnDidChangeContent(handle, () => listener({ changes: [], eol: "\n", versionId: model.getVersionId() }))
          return { dispose }
        }
        return { dispose() {} }
      },
      getLinesContent() {
        return lil.modelGetLinesContent ? lil.modelGetLinesContent(handle) : [model.getValue()]
      },
      getValueLength() {
        return lil.modelGetValueLength ? lil.modelGetValueLength(handle) : model.getValue().length
      },
      getLineMaxColumn(line) {
        return model.getLineLength(line) + 1
      },
      getLineMinColumn() {
        return 1
      },
      getLineFirstNonWhitespaceColumn(line) {
        const text = model.getLineContent(line)
        const m = text.match(/\S/)
        return m ? m.index + 1 : 0
      },
      getLineLastNonWhitespaceColumn(line) {
        const text = model.getLineContent(line)
        const m = text.match(/\S\s*$/)
        return m ? m.index + 2 : 0
      },
      pushStackElement() {
        lil.modelPushStackElement?.(handle)
      },
      pushUndoStop() {
        lil.modelPushStackElement?.(handle)
        return true
      },
      popUndoStop() {
        return false
      },
      findNextMatch(query, start, isRegex, matchCase, wholeWord) {
        const packed = lil.modelFindNextMatchPacked
          ? lil.modelFindNextMatchPacked(handle, query ?? "", start?.lineNumber ?? 1, start?.column ?? 1, !!isRegex, matchCase !== false, !!wholeWord)
          : []
        if (!packed || packed.length < 4) {
          return null
        }
        return { range: Range(packed[0], packed[1], packed[2], packed[3]), matches: [query] }
      },
      findPreviousMatch(query, start, isRegex, matchCase, wholeWord) {
        const hits = model.findMatches(query, false, isRegex, matchCase, wholeWord, false, 1000)
        let last = null
        for (const hit of hits) {
          const r = hit.range ?? hit
          if (r.startLineNumber < start.lineNumber || (r.startLineNumber === start.lineNumber && r.startColumn < start.column)) {
            last = hit
          }
        }
        return last
      },
      getWordUntilPosition(pos) {
        const word = model.getWordAtPosition(pos)
        if (!word) {
          return { word: "", startColumn: pos.column, endColumn: pos.column }
        }
        return { word: word.word.slice(0, Math.max(0, pos.column - word.startColumn)), startColumn: word.startColumn, endColumn: pos.column }
      },
      validatePosition(pos) {
        const last = model.getLineCount()
        let line = pos.lineNumber | 0
        if (line < 1) line = 1
        if (line > last) line = last
        let col = pos.column | 0
        const max = model.getLineMaxColumn(line)
        if (col < 1) col = 1
        if (col > max) col = max
        return Position(line, col)
      },
      validateRange(range) {
        const s = model.validatePosition({ lineNumber: range.startLineNumber, column: range.startColumn })
        const e = model.validatePosition({ lineNumber: range.endLineNumber, column: range.endColumn })
        return Range(s.lineNumber, s.column, e.lineNumber, e.column)
      },
      modifyPosition(pos, offset) {
        return model.getPositionAt(model.getOffsetAt(pos) + (offset | 0))
      },
      getOptions() {
        return { tabSize: 4, insertSpaces: true, defaultEOL: 1, trimAutoWhitespace: true, indentSize: 4, bracketPairColorizationOptions: { enabled: false } }
      },
      getFormattingOptions() {
        return { tabSize: 4, insertSpaces: true }
      },
      detectIndentation() {},
      normalizeIndentation(text) {
        return text
      },
      updateOptions() {},
      isDisposed() {
        return false
      },
      isAttachedToEditor() {
        return true
      },
      getAlternativeVersionId() {
        return model.getVersionId()
      },
      setEOL() {},
      getEndOfLineSequence() {
        return 0
      },
      onDidChangeLanguage(listener) {
        return modelLanguage((e) => {
          if (e.model === model) listener(e)
        })
      },
      onWillDispose(listener) {
        return modelDisposed((m) => {
          if (m === model) listener()
        })
      },
      getAllDecorations() {
        return []
      },
      getDecorationsInRange() {
        return []
      },
      getLineDecorations() {
        return []
      },
      getOverviewRulerDecorations() {
        return []
      },
      getInjectedTextDecorations() {
        return []
      },
      changeDecorations(cb) {
        const acc = {
          addDecoration(range, options) {
            const ids = model.deltaDecorations([], [{ range, options }])
            return ids[0]
          },
          changeDecoration() {},
          changeDecorationOptions() {},
          removeDecoration(id) {
            model.deltaDecorations([id], [])
          },
          deltaDecorations(oldIds, next) {
            return model.deltaDecorations(oldIds, next)
          },
        }
        return cb(acc)
      },
    }
    modelCache.set(handle, model)
    return model
  }

  function wrapEditor(handle) {
    if (!handle) {
      return null
    }
    if (handle._handle && handle.trigger) {
      return handle
    }
    const cached = editorCache.get(handle)
    if (cached) {
      return cached
    }
    const localActions = new Map()
    const listeners = {
      content: [],
      cursor: [],
    }
    if (typeof lil.editorOnDidChangeModelContent === "function") {
      lil.editorOnDidChangeModelContent(handle, () => {
        for (const fn of listeners.content) {
          fn({})
        }
      })
    }
    if (typeof lil.editorOnDidChangeCursorPosition === "function") {
      lil.editorOnDidChangeCursorPosition(handle, () => {
        for (const fn of listeners.cursor) {
          fn({ position: lil.editorGetPosition(handle) })
        }
      })
    }
    const wrapped = {
      _handle: handle,
      getValue() {
        return lil.editorGetValue(handle)
      },
      setValue(value) {
        lil.editorSetValue(handle, value ?? "")
      },
      getModel() {
        return wrapModel(lil.editorGetModel(handle))
      },
      setModel(model) {
        lil.editorSetModel(handle, unwrap(model))
        if (typeof lil.editorSetModelFacade === "function") {
          lil.editorSetModelFacade(handle, wrapModel(unwrap(model)))
        }
      },
      getPosition() {
        if (typeof lil.editorGetSelectionsPacked === "function") {
          const packed = lil.editorGetSelectionsPacked(handle)
          if (packed && packed.length >= 4) {
            return { lineNumber: packed[2] | 0 || 1, column: packed[3] | 0 || 1 }
          }
        }
        return asPosition(lil.editorGetPosition(handle))
      },
      setPosition(pos) {
        lil.editorSetPosition(handle, lilPos(pos))
      },
      getSelection() {
        return lil.editorGetSelection(handle)
      },
      setSelection(sel) {
        lil.editorSetSelection(handle, sel)
      },
      trigger(source, handlerId, payload) {
        const local = localActions.get(handlerId)
        if (typeof local?.run === "function") {
          return local.run(wrapped, payload)
        }
        lil.editorTrigger(handle, source, handlerId, payload ?? {})
      },
      layout(dimension) {
        if (dimension && typeof lil.editorLayoutSize === "function") {
          lil.editorLayoutSize(handle, dimension.width | 0, dimension.height | 0)
          return
        }
        lil.editorLayout(handle)
      },
      focus() {
        lil.editorFocus(handle)
      },
      hasTextFocus() {
        return true
      },
      dispose() {
        lil.editorDispose(handle)
      },
      executeEdits(_source, edits) {
        const ops = (edits ?? []).map((e) =>
          lil.editOp(
            e.range.startLineNumber,
            e.range.startColumn,
            e.range.endLineNumber,
            e.range.endColumn,
            e.text ?? "",
          ),
        )
        return lil.editorExecuteEdits(handle, ops)
      },
      deltaDecorations(oldIds, next) {
        const decos = (next ?? []).map((d) =>
          lil.deco(
            d.range.startLineNumber,
            d.range.startColumn,
            d.range.endLineNumber,
            d.range.endColumn,
            d.options?.inlineClassName ?? d.options?.className ?? "",
          ),
        )
        return lil.editorDeltaDecorations(handle, oldIds ?? [], decos)
      },
      revealLine(line) {
        lil.editorRevealLine(handle, line)
      },
      revealLineInCenter(line) {
        lil.editorRevealLine(handle, line)
      },
      revealPosition(pos) {
        lil.editorSetPosition(handle, pos)
      },
      revealRange(range) {
        lil.editorRevealLine(handle, range.startLineNumber)
      },
      addAction(desc) {
        localActions.set(desc.id, desc)
        lil.editorAddAction?.(handle, desc.id, desc.label ?? desc.id, desc.run)
        return {
          dispose() {
            localActions.delete(desc.id)
          },
        }
      },
      addCommand(_keybinding, handler) {
        const id = "cmd." + Math.random().toString(36).slice(2)
        lil.editorAddAction(handle, id, id, handler)
        return id
      },
      getAction(id) {
        return {
          id,
          run() {
            return wrapped.trigger("action", id, {})
          },
        }
      },
      updateOptions(options) {
        lil.editorUpdateOptions?.(handle, options ?? {})
      },
      getOptions() {
        return {
          get() {
            return undefined
          },
        }
      },
      saveViewState() {
        const pos = lil.editorGetPosition(handle)
        return { cursorState: [{ inSelectionMode: false, position: pos }] }
      },
      restoreViewState(state) {
        const pos = state?.cursorState?.[0]?.position
        if (pos) {
          lil.editorSetPosition(handle, pos)
        }
      },
      onDidChangeModelContent(listener) {
        listeners.content.push(listener)
        return {
          dispose() {
            listeners.content = listeners.content.filter((fn) => fn !== listener)
          },
        }
      },
      onDidChangeCursorPosition(listener) {
        listeners.cursor.push(listener)
        return {
          dispose() {
            listeners.cursor = listeners.cursor.filter((fn) => fn !== listener)
          },
        }
      },
      onDidChangeModel(listener) {
        return { dispose() {} }
      },
      onDidFocusEditorText(listener) {
        return { dispose() {} }
      },
      onDidBlurEditorText(listener) {
        return { dispose() {} }
      },
      onDidChangeCursorSelection(listener) {
        return wrapped.onDidChangeCursorPosition((e) => listener({ selection: wrapped.getSelection(), ...e }))
      },
      onDidChangeModelLanguage: eventHub(),
      onDidChangeModelLanguageConfiguration: eventHub(),
      onDidChangeModelOptions: eventHub(),
      onDidChangeConfiguration: eventHub(),
      onWillChangeModel: eventHub(),
      onDidChangeModelDecorations: eventHub(),
      onDidFocusEditorWidget: eventHub(),
      onDidBlurEditorWidget: eventHub(),
      onDidCompositionStart: eventHub(),
      onDidCompositionEnd: eventHub(),
      onDidAttemptReadOnlyEdit: eventHub(),
      onDidPaste: eventHub(),
      onMouseUp: eventHub(),
      onMouseDown: eventHub(),
      onContextMenu: eventHub(),
      onMouseMove: eventHub(),
      onMouseLeave: eventHub(),
      onKeyUp: eventHub(),
      onKeyDown: eventHub(),
      onDidLayoutChange: eventHub(),
      onDidContentSizeChange: eventHub(),
      onDidScrollChange: eventHub(),
      onDidChangeHiddenAreas: eventHub(),
      onBeginUpdate: eventHub(),
      onEndUpdate: eventHub(),
      onDidChangeViewZones: eventHub(),
      inComposition: false,
      getId() {
        if (!wrapped._id) {
          editorSeq += 1
          wrapped._id = "code-" + editorSeq
        }
        return wrapped._id
      },
      getEditorType() {
        return "vs.editor.ICodeEditor"
      },
      getContainerDomNode() {
        return lil.editorGetDomNode ? lil.editorGetDomNode(handle) : null
      },
      getDomNode() {
        return wrapped.getContainerDomNode()
      },
      getOverflowWidgetsDomNode() {
        return lil.editorGetOverflowWidgetsDomNode ? lil.editorGetOverflowWidgetsDomNode(handle) : null
      },
      getScrollTop() {
        return lil.editorGetScrollTop ? lil.editorGetScrollTop(handle) : 0
      },
      setScrollTop(value) {
        lil.editorSetScrollTop?.(handle, value | 0)
      },
      getScrollLeft() {
        return lil.editorGetScrollLeft ? lil.editorGetScrollLeft(handle) : 0
      },
      setScrollLeft(value) {
        lil.editorSetScrollLeft?.(handle, value | 0)
      },
      setScrollPosition(pos) {
        if (pos && pos.scrollTop != null) wrapped.setScrollTop(pos.scrollTop)
        if (pos && pos.scrollLeft != null) wrapped.setScrollLeft(pos.scrollLeft)
      },
      hasPendingScrollAnimation() {
        return false
      },
      getContentHeight() {
        return lil.editorGetContentHeight ? lil.editorGetContentHeight(handle) : 0
      },
      getContentWidth() {
        return lil.editorGetContentWidth ? lil.editorGetContentWidth(handle) : 0
      },
      getScrollHeight() {
        return wrapped.getContentHeight()
      },
      getScrollWidth() {
        return wrapped.getContentWidth()
      },
      getVisibleRanges() {
        const packed = lil.editorGetVisibleRangePacked ? lil.editorGetVisibleRangePacked(handle) : [1, 1, 1, 1]
        return [Range(packed[0], packed[1], packed[2], packed[3])]
      },
      getSelections() {
        if (lil.editorGetSelectionsPacked) {
          const sels = unpackSels(lil.editorGetSelectionsPacked(handle))
          return sels.length ? sels : [wrapped.getSelection()]
        }
        return [wrapped.getSelection()]
      },
      setSelections(sels) {
        if (lil.editorSetSelectionsPacked) {
          lil.editorSetSelectionsPacked(handle, packSels(sels))
          return
        }
        if (sels && sels[0]) {
          wrapped.setSelection(sels[0])
        }
      },
      getTopForLineNumber(line) {
        return ((line | 0) - 1) * 19
      },
      getBottomForLineNumber(line) {
        return (line | 0) * 19
      },
      getTopForPosition(line) {
        return wrapped.getTopForLineNumber(line)
      },
      getLineHeightForPosition() {
        return 19
      },
      getOffsetForColumn(_line, column) {
        return ((column | 0) - 1) * 8
      },
      getScrolledVisiblePosition(pos) {
        return { top: wrapped.getTopForLineNumber(pos.lineNumber) - wrapped.getScrollTop(), left: wrapped.getOffsetForColumn(pos.lineNumber, pos.column), height: 19 }
      },
      getTargetAtClientPoint() {
        return null
      },
      createDecorationsCollection(decos) {
        let ids = wrapped.deltaDecorations([], decos ?? [])
        return {
          set(next) {
            ids = wrapped.deltaDecorations(ids, next ?? [])
          },
          clear() {
            ids = wrapped.deltaDecorations(ids, [])
          },
          getRanges() {
            return []
          },
          has() {
            return ids.length > 0
          },
          length: ids.length,
        }
      },
      removeDecorations(ids) {
        wrapped.deltaDecorations(ids ?? [], [])
      },
      getLineDecorations() {
        return []
      },
      getDecorationsInRange() {
        return []
      },
      createContextKey(key, value) {
        let current = value
        return {
          set(next) {
            current = next
          },
          get() {
            return current
          },
          reset() {
            current = value
          },
        }
      },
      addContentWidget(widget) {
        const host = wrapped.getOverflowWidgetsDomNode()
        const node = widget?.getDomNode?.()
        if (host && node && !node.parentNode) {
          host.appendChild(node)
        }
      },
      layoutContentWidget() {},
      removeContentWidget(widget) {
        widget?.getDomNode?.()?.remove?.()
      },
      addOverlayWidget(widget) {
        wrapped.addContentWidget(widget)
      },
      layoutOverlayWidget() {},
      removeOverlayWidget(widget) {
        wrapped.removeContentWidget(widget)
      },
      addGlyphMarginWidget() {},
      layoutGlyphMarginWidget() {},
      removeGlyphMarginWidget() {},
      changeViewZones() {},
      render() {
        lil.editorLayout?.(handle)
      },
      renderAsync() {
        wrapped.render()
      },
      applyFontInfo() {},
      setBanner() {},
      writeScreenReaderContent() {},
      hasWidgetFocus() {
        return true
      },
      getContribution() {
        return null
      },
      getSupportedActions() {
        const ids = new Set([
          "undo",
          "redo",
          "actions.find",
          "editor.action.startFindReplaceAction",
          ...localActions.keys(),
        ])
        return [...ids].map((id) => wrapped.getAction(id))
      },
      executeCommand(_source, command) {
        if (command && typeof command.getEditOperations === "function") {
          return
        }
      },
      executeCommands() {},
      pushUndoStop() {
        const model = wrapped.getModel()
        model?.pushUndoStop?.()
        return true
      },
      popUndoStop() {
        return false
      },
      revealAllCursors() {
        const pos = wrapped.getPosition()
        if (pos) wrapped.revealLine(pos.lineNumber)
      },
      revealLineInCenterIfOutsideViewport(line) {
        wrapped.revealLine(line)
      },
      revealPositionInCenter(pos) {
        wrapped.revealPosition(pos)
      },
      revealRangeInCenter(range) {
        wrapped.revealRange(range)
      },
      revealRangeAtTop(range) {
        wrapped.revealRange(range)
      },
      getLayoutInfo() {
        return {
          width: wrapped.getContentWidth(),
          height: wrapped.getContentHeight(),
          glyphMarginLeft: 0,
          glyphMarginWidth: 0,
          lineNumbersLeft: 0,
          lineNumbersWidth: 40,
          decorationsLeft: 40,
          decorationsWidth: 10,
          contentLeft: 56,
          contentWidth: wrapped.getContentWidth(),
          minimap: { renderMinimap: 0, minimapLeft: 0, minimapWidth: 0 },
        }
      },
      getRawOptions() {
        return {}
      },
      getOption() {
        return undefined
      },
      getConfiguredWordAtPosition(pos) {
        return wrapped.getModel()?.getWordAtPosition(pos) ?? null
      },
      getFontSizeAtPosition() {
        return "14px"
      },
    }
    editorCache.set(handle, wrapped)
    if (typeof lil.editorSetModelFacade === "function") {
      lil.editorSetModelFacade(handle, wrapped.getModel())
    }
    for (const action of globalActions) {
      wrapped.addAction(action)
    }
    return wrapped
  }

  function wrapDiff(handle) {
    const original = wrapEditor(lil.diffGetOriginal ? lil.diffGetOriginal(handle) : handle)
    const modified = wrapEditor(lil.diffGetModified ? lil.diffGetModified(handle) : handle)
    return {
      _handle: handle,
      getOriginalEditor() {
        return original
      },
      getModifiedEditor() {
        return modified
      },
      getLineChanges() {
        return typeof lil.diffLineChanges === "function" ? lil.diffLineChanges(handle) : []
      },
      getDiffLineInformationForOriginal() {
        return null
      },
      getDiffLineInformationForModified() {
        return null
      },
      setModel(model) {
        if (model?.original) {
          original.setModel(model.original)
        }
        if (model?.modified) {
          modified.setModel(model.modified)
        }
      },
      getModel() {
        return { original: original.getModel(), modified: modified.getModel() }
      },
      layout() {
        lil.diffLayout?.(handle)
        original.layout()
        modified.layout()
      },
      updateOptions() {},
      onDidUpdateDiff: eventHub(),
      onDidChangeModel: eventHub(),
      getContainerDomNode() {
        return original.getContainerDomNode()
      },
      getEditorType() {
        return "vs.editor.IDiffEditor"
      },
      addCommand(keybinding, handler) {
        return original.addCommand(keybinding, handler)
      },
      createContextKey(key, value) {
        return original.createContextKey(key, value)
      },
      addAction(desc) {
        original.addAction(desc)
        return modified.addAction(desc)
      },
      dispose() {
        lil.diffDispose(handle)
      },
    }
  }

  const editor = {
    create(dom, options = {}) {
      const next = { ...options }
      const existing = next.model ? unwrap(next.model) : null
      if (existing) {
        next.value = next.value ?? (lil.modelGetValue ? lil.modelGetValue(existing) : "")
        next.language = next.language ?? (lil.modelGetLanguageId ? lil.modelGetLanguageId(existing) : "plaintext")
      }
      const handle = lil.create(dom, next)
      const wrapped = wrapEditor(handle)
      if (existing) {
        const temp = wrapped.getModel()
        wrapped.setModel(wrapModel(existing))
        temp?.dispose?.()
      }
      return wrapped
    },
    createDiffEditor(dom, options) {
      return wrapDiff(lil.createDiffEditor(dom, options ?? {}))
    },
    createModel(value, language, uri) {
      const wrapped = uri && lil.createModelWithUri
        ? wrapModel(lil.createModelWithUri(value ?? "", language ?? "plaintext", uri))
        : wrapModel(lil.createModel(value ?? "", language ?? "plaintext"))
      modelCreated._fire(wrapped)
      return wrapped
    },
    setTheme(name) {
      lil.setTheme(name)
    },
    defineTheme(name, data) {
      lil.defineTheme(name, data)
    },
    setModelLanguage(model, languageId) {
      const oldLanguage = model?.getLanguageId?.() ?? ""
      lil.setModelLanguage(unwrap(model), languageId)
      modelLanguage._fire({ model, oldLanguage })
    },
    setModelMarkers(model, owner, markers) {
      lil.setModelMarkers(unwrap(model), owner, markers ?? [])
      markersChanged._fire([model?.uri].filter(Boolean))
    },
    getModelMarkers(filter = {}) {
      return lil.getModelMarkers(filter.owner ?? "", filter.resource ?? "", filter.take ?? 0)
    },
    removeAllMarkers(owner) {
      lil.removeAllMarkers(owner)
    },
    getModels() {
      return (lil.getModels?.() ?? []).map(wrapModel)
    },
    getModel(uri) {
      const models = editor.getModels()
      const key = uri?.toString?.() ?? String(uri ?? "")
      return models.find((m) => (m.uri?.toString?.() ?? "") === key) ?? null
    },
    getEditors() {
      return (lil.getEditors?.() ?? []).map(wrapEditor)
    },
    getDiffEditors() {
      return (lil.getDiffEditors?.() ?? []).map(wrapDiff)
    },
    onDidCreateEditor(listener) {
      if (typeof lil.onDidCreateEditor === "function") {
        return disposable(lil.onDidCreateEditor(() => listener({})))
      }
      return disposable()
    },
    onDidCreateDiffEditor(listener) {
      if (typeof lil.onDidCreateDiffEditor === "function") {
        return disposable(lil.onDidCreateDiffEditor(() => listener({})))
      }
      return disposable()
    },
    createMultiFileDiffEditor(dom, override) {
      return editor.createDiffEditor(dom, override)
    },
    addCommand(descriptor) {
      globalCommands.push(descriptor)
      return disposable(() => {
        const i = globalCommands.indexOf(descriptor)
        if (i >= 0) globalCommands.splice(i, 1)
      })
    },
    addEditorAction(descriptor) {
      globalActions.push(descriptor)
      for (const ed of editor.getEditors()) {
        ed.addAction(descriptor)
      }
      return disposable(() => {
        const i = globalActions.indexOf(descriptor)
        if (i >= 0) globalActions.splice(i, 1)
      })
    },
    addKeybindingRule(rule) {
      keybindingRules.push(rule)
      return disposable()
    },
    addKeybindingRules(rules) {
      for (const rule of rules ?? []) {
        editor.addKeybindingRule(rule)
      }
      return disposable()
    },
    onDidChangeMarkers(listener) {
      return markersChanged(listener)
    },
    onDidCreateModel(listener) {
      return modelCreated(listener)
    },
    onWillDisposeModel(listener) {
      return modelDisposed(listener)
    },
    onDidChangeModelLanguage(listener) {
      return modelLanguage(listener)
    },
    createWebWorker() {
      return {
        getProxy() {
          return Promise.resolve({})
        },
        withSyncedResources() {
          return Promise.resolve({})
        },
        dispose() {},
      }
    },
    colorize(text, languageId) {
      const rows = editor.tokenize(text, languageId)
      const lines = String(text ?? "").split(/\r\n|\r|\n/)
      const html = lines.map((line, i) => {
        const tokens = rows[i] ?? []
        if (!tokens.length) {
          return escapeHtml(line)
        }
        let out = ""
        let last = 0
        for (const token of tokens) {
          const start = token.offset ?? token.startIndex ?? last
          if (start > last) {
            out += escapeHtml(line.slice(last, start))
          }
          const cls = String(token.type ?? token.scopes ?? "source").replace(/\./g, " ")
          const end = tokens[tokens.indexOf(token) + 1]?.offset ?? line.length
          out += `<span class="mtk ${cls}">${escapeHtml(line.slice(start, end))}</span>`
          last = end
        }
        if (last < line.length) {
          out += escapeHtml(line.slice(last))
        }
        return out
      }).join("<br/>")
      return Promise.resolve(html)
    },
    colorizeElement(domNode, options) {
      const lang = domNode?.getAttribute?.("data-lang") ?? options?.theme ?? "plaintext"
      return editor.colorize(domNode?.textContent ?? "", lang, options ?? {}).then((html) => {
        if (domNode) domNode.innerHTML = html
      })
    },
    colorizeModelLine(model, lineNumber) {
      const line = model.getLineContent(lineNumber)
      const rows = editor.tokenize(line, model.getLanguageId())
      return rows[0] ? rows[0].map((t) => t.type).join(" ") : line
    },
    tokenize(text, languageId) {
      const packed = typeof lil.tokenizePacked === "function" ? lil.tokenizePacked(languageId, text) : []
      const lines = String(text ?? "").split(/\r\n|\r|\n/)
      const rows = lines.map(() => [])
      let lineStart = 0
      let line = 0
      for (const row of packed) {
        const sep = String(row).indexOf(":")
        const offset = Number(String(row).slice(0, sep))
        const type = String(row).slice(sep + 1)
        while (line < lines.length - 1 && offset >= lineStart + lines[line].length + 1) {
          lineStart += lines[line].length + 1
          line += 1
        }
        rows[line].push({ offset: offset - lineStart, type, language: languageId })
      }
      return rows
    },
    remeasureFonts() {},
    registerCommand(id, handler) {
      return editor.addCommand({ id, run: handler })
    },
    registerLinkOpener(opener) {
      linkOpeners.push(opener)
      return disposable(() => {
        const i = linkOpeners.indexOf(opener)
        if (i >= 0) linkOpeners.splice(i, 1)
      })
    },
    registerEditorOpener(opener) {
      editorOpeners.push(opener)
      return disposable(() => {
        const i = editorOpeners.indexOf(opener)
        if (i >= 0) editorOpeners.splice(i, 1)
      })
    },
    EditorType: {
      ICodeEditor: "vs.editor.ICodeEditor",
      IDiffEditor: "vs.editor.IDiffEditor",
    },
    ScrollType: { Smooth: 0, Immediate: 1 },
    EndOfLineSequence: { LF: 0, CRLF: 1 },
    DefaultEndOfLine: { LF: 1, CRLF: 2 },
    EndOfLinePreference: { TextDefined: 0, LF: 1, CRLF: 2 },
    TrackedRangeStickiness: { AlwaysGrowsWhenTypingAtEdges: 0, NeverGrowsWhenTypingAtEdges: 1, GrowsOnlyWhenTypingBefore: 2, GrowsOnlyWhenTypingAfter: 3 },
    OverviewRulerLane: { Left: 1, Center: 2, Right: 4, Full: 7 },
    MinimapPosition: { Inline: 1, Gutter: 2 },
    GlyphMarginLane: { Left: 1, Center: 2, Right: 3 },
    RenderMinimap: { None: 0, Text: 1, Blocks: 2 },
    TextEditorCursorStyle: { Line: 1, Block: 2, Underline: 3, LineThin: 4, BlockOutline: 5, UnderlineThin: 6 },
    TextEditorCursorBlinkingStyle: { Hidden: 0, Blink: 1, Smooth: 2, Phase: 3, Expand: 4, Solid: 5 },
    WrappingIndent: { None: 0, Same: 1, Indent: 2, DeepIndent: 3 },
    RenderLineNumbersType: { Off: 0, On: 1, Relative: 2, Interval: 3, Custom: 4 },
    AccessibilitySupport: { Unknown: 0, Disabled: 1, Enabled: 2 },
  }

  const languages = {
    register(lang) {
      if (lang?.id && typeof lil.registerLanguageId === "function") {
        lil.registerLanguageId(lang.id)
        const pending = languageEncountered.get(lang.id)
        if (pending) {
          for (const fn of pending) fn()
          languageEncountered.delete(lang.id)
        }
      }
    },
    getLanguages() {
      return (lil.languageIds?.() ?? []).map((id) => ({ id }))
    },
    getEncodedLanguageId(languageId) {
      let hash = 0
      for (let i = 0; i < String(languageId ?? "").length; i++) {
        hash = (hash + languageId.charCodeAt(i)) & 255
      }
      return hash || 1
    },
    onLanguage(languageId, callback) {
      const ids = lil.languageIds?.() ?? []
      if (ids.includes(languageId)) {
        callback()
        return disposable()
      }
      const list = languageEncountered.get(languageId) ?? []
      list.push(callback)
      languageEncountered.set(languageId, list)
      return disposable(() => {
        const next = (languageEncountered.get(languageId) ?? []).filter((fn) => fn !== callback)
        languageEncountered.set(languageId, next)
      })
    },
    onLanguageEncountered(languageId, callback) {
      return languages.onLanguage(languageId, callback)
    },
    registerCompletionItemProvider(selector, provider) {
      return { dispose: lil.languagesRegisterCompletion(selector, provider) }
    },
    registerHoverProvider(selector, provider) {
      return { dispose: lil.languagesRegisterHover(selector, provider) }
    },
    registerDefinitionProvider(selector, provider) {
      return { dispose: lil.languagesRegisterDefinition(selector, provider) }
    },
    registerReferenceProvider(selector, provider) {
      return { dispose: lil.languagesRegisterReference(selector, provider) }
    },
    registerDocumentSymbolProvider(selector, provider) {
      return { dispose: lil.languagesRegisterDocumentSymbol(selector, provider) }
    },
    registerDocumentFormattingEditProvider(selector, provider) {
      return { dispose: lil.languagesRegisterFormatting(selector, provider) }
    },
    registerDocumentRangeFormattingEditProvider(selector, provider) {
      return { dispose: lil.languagesRegisterFormatting(selector, provider) }
    },
    registerRenameProvider(selector, provider) {
      return { dispose: lil.languagesRegisterRename(selector, provider) }
    },
    registerSignatureHelpProvider(selector, provider) {
      return { dispose: lil.languagesRegisterSignatureHelp(selector, provider) }
    },
    registerFoldingRangeProvider(selector, provider) {
      return { dispose: lil.languagesRegisterFolding(selector, provider) }
    },
    registerLinkProvider(selector, provider) {
      return { dispose: lil.languagesRegisterLink(selector, provider) }
    },
    registerCodeActionProvider(selector, provider) {
      return { dispose: lil.languagesRegisterCodeAction(selector, provider) }
    },
    registerCodeLensProvider(selector, provider) {
      return { dispose: lil.languagesRegisterCodeLens(selector, provider) }
    },
    registerColorProvider(selector, provider) {
      return { dispose: lil.languagesRegisterColor(selector, provider) }
    },
    registerDocumentHighlightProvider(selector, provider) {
      return { dispose: lil.languagesRegisterHighlight(selector, provider) }
    },
    registerInlayHintsProvider(selector, provider) {
      return { dispose: lil.languagesRegisterInlayHints(selector, provider) }
    },
    registerInlineCompletionsProvider(selector, provider) {
      return { dispose: lil.languagesRegisterInlineCompletions(selector, provider) }
    },
    registerImplementationProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("implementation", selector, provider) : () => {} }
    },
    registerTypeDefinitionProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("typeDefinition", selector, provider) : () => {} }
    },
    registerDeclarationProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("declaration", selector, provider) : () => {} }
    },
    registerSelectionRangeProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("selectionRange", selector, provider) : () => {} }
    },
    registerLinkedEditingRangeProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("linkedEditing", selector, provider) : () => {} }
    },
    registerOnTypeFormattingEditProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("onTypeFormatting", selector, provider) : () => {} }
    },
    registerDocumentSemanticTokensProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("documentSemanticTokens", selector, provider) : () => {} }
    },
    registerDocumentRangeSemanticTokensProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("documentRangeSemanticTokens", selector, provider) : () => {} }
    },
    registerNewSymbolNameProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("newSymbolName", selector, provider) : () => {} }
    },
    registerTokensProviderFactory(languageId, factory) {
      Promise.resolve(factory?.create?.()).then((provider) => {
        if (provider) languages.setTokensProvider(languageId, provider)
      })
      return disposable()
    },
    setColorMap(map) {
      colorMap = map
    },
    setLanguageConfiguration(id, config) {
      lil.setLanguageConfigurationJs?.(id, config)
      return { dispose() {} }
    },
    setMonarchTokensProvider(id, def) {
      const provider = compileMonarch(id, def)
      lil.setTokensProviderJs?.(id, provider)
      return { dispose() {} }
    },
    setTokensProvider(id, provider) {
      const tokenize = typeof provider?.tokenize === "function"
        ? (line, state, _c) => {
            const out = provider.tokenize(line, state)
            return out
          }
        : provider
      lil.setTokensProviderJs?.(id, typeof tokenize === "function" ? { tokenize } : provider)
      return { dispose() {} }
    },
    CompletionItemKind,
    CompletionItemInsertTextRule: { None: 0, KeepWhitespace: 1, InsertAsSnippet: 4 },
    CompletionItemTag: { Deprecated: 1 },
    CompletionTriggerKind: { Invoke: 0, TriggerCharacter: 1, TriggerForIncompleteCompletions: 2 },
    DocumentHighlightKind: { Text: 0, Read: 1, Write: 2 },
    SymbolKind: {
      File: 0, Module: 1, Namespace: 2, Package: 3, Class: 4, Method: 5, Property: 6, Field: 7,
      Constructor: 8, Enum: 9, Interface: 10, Function: 11, Variable: 12, Constant: 13, String: 14,
      Number: 15, Boolean: 16, Array: 17, Object: 18, Key: 19, Null: 20, EnumMember: 21, Struct: 22,
      Event: 23, Operator: 24, TypeParameter: 25,
    },
    SymbolTag: { Deprecated: 1 },
    IndentAction: { None: 0, Indent: 1, IndentOutdent: 2, Outdent: 3 },
    FoldingRangeKind: { Comment: { value: "comment" }, Imports: { value: "imports" }, Region: { value: "region" } },
    SignatureHelpTriggerKind: { Invoke: 1, TriggerCharacter: 2, ContentChange: 3 },
    InlayHintKind: { Type: 1, Parameter: 2 },
    InlineCompletionTriggerKind: { Automatic: 0, Explicit: 1 },
    NewSymbolNameTriggerKind: { Invoke: 0, Automatic: 1 },
  }

  return {
    editor,
    languages,
    Range,
    Position,
    Selection,
    Uri: Object.assign(
      function Uri(scheme, authority, path, query, fragment) {
        const raw = `${scheme}://${authority ?? ""}${path ?? ""}${query ? "?" + query : ""}${fragment ? "#" + fragment : ""}`
        return lil.parseUri ? lil.parseUri(raw) : { scheme, authority, path, query, fragment, toString: () => raw }
      },
      {
        parse(value) {
          return lil.parseUri ? lil.parseUri(String(value ?? "")) : { scheme: "", path: String(value ?? ""), toString: () => String(value ?? "") }
        },
        file(path) {
          return lil.fileUri ? lil.fileUri(String(path ?? "")) : { scheme: "file", path: String(path ?? ""), toString: () => "file://" + path }
        },
      },
    ),
    KeyCode: lil.KeyCode,
    KeyMod,
    MarkerSeverity,
    MarkerTag: { Unnecessary: 1, Deprecated: 2 },
    CompletionItemKind,
    CancellationTokenSource,
    Emitter,
    Token: function Token(offset, type, language) {
      this.offset = offset
      this.type = type
      this.language = language
    },
    bindMonaco,
  }
}

export { bindMonaco as default }
