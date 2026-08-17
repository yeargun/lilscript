var __defProp = Object.defineProperty;
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};

// ports/monaco/monaco-api.ts
function compileMonarch(languageId, def) {
  const spec = def && typeof def.then !== "function" ? def : { tokenizer: { root: [] } };
  const tokenizer = spec.tokenizer ?? {};
  const keywords = spec.keywords ?? [];
  const postfix = spec.tokenPostfix ?? "." + languageId;
  const defaultToken = spec.defaultToken ?? "source";
  function expandInclude(state, seen) {
    const rules = tokenizer[state] ?? [];
    const out = [];
    for (const rule of rules) {
      if (rule && typeof rule === "object" && typeof rule.include === "string") {
        const name = rule.include.replace(/^@/, "");
        if (!seen.has(name)) {
          seen.add(name);
          out.push(...expandInclude(name, seen));
        }
        continue;
      }
      out.push(rule);
    }
    return out;
  }
  function tokenName(token, text) {
    if (token == null) {
      return defaultToken;
    }
    if (typeof token === "string") {
      if (token === "@brackets") {
        return "delimiter.bracket" + postfix;
      }
      if (token.indexOf(".") >= 0 || postfix.length === 0) {
        return token;
      }
      return token + postfix;
    }
    if (token.cases) {
      const word = text;
      if (token.cases["@keywords"] && keywords.includes(word)) {
        return tokenName(token.cases["@keywords"], word);
      }
      if (token.cases["@default"]) {
        return tokenName(token.cases["@default"], word);
      }
      return defaultToken;
    }
    if (typeof token.token === "string") {
      return tokenName(token.token, text);
    }
    return defaultToken;
  }
  function nextState(rule, stack) {
    const action = Array.isArray(rule) ? rule[1] : rule;
    if (!action || typeof action === "string") {
      return stack;
    }
    const next = action.next ?? action.switchTo;
    if (typeof next !== "string") {
      return stack;
    }
    const copy = stack.slice();
    if (next === "@pop") {
      if (copy.length > 1) {
        copy.pop();
      }
      return copy;
    }
    if (next.startsWith("@")) {
      copy.push(next.slice(1));
      return copy;
    }
    copy[copy.length - 1] = next;
    return copy;
  }
  function toRegex(rule) {
    const pat = Array.isArray(rule) ? rule[0] : rule.regex;
    if (pat instanceof RegExp) {
      const src2 = pat.source.startsWith("^") ? pat.source : "^(?:" + pat.source + ")";
      return new RegExp(src2);
    }
    const src = String(pat ?? "");
    return new RegExp(src.startsWith("^") ? src : "^(?:" + src + ")");
  }
  return {
    tokenize(line) {
      const tokens = [];
      let pos = 0;
      let stack = ["root"];
      let guard = 0;
      while (pos < line.length && guard < 1e4) {
        guard++;
        const state = stack[stack.length - 1] ?? "root";
        const rules = expandInclude(state, /* @__PURE__ */ new Set());
        let matched = "";
        let matchedRule = null;
        for (const rule of rules) {
          const re2 = toRegex(rule);
          const found = re2.exec(line.slice(pos));
          if (found && found[0] != null) {
            matched = found[0];
            matchedRule = rule;
            break;
          }
        }
        if (!matchedRule) {
          matched = line.charAt(pos);
          tokens.push({ offset: pos, type: defaultToken });
          pos += 1;
          continue;
        }
        if (matched.length === 0) {
          matched = line.charAt(pos) || "";
          if (!matched) {
            break;
          }
        }
        const action = Array.isArray(matchedRule) ? matchedRule[1] : matchedRule;
        const type = tokenName(action, matched);
        if (type && type !== "@rematch") {
          tokens.push({ offset: pos, type });
          pos += matched.length;
        }
        stack = nextState(matchedRule, stack);
      }
      return { tokens, endState: stack };
    }
  };
}
function bindMonaco(lil) {
  if (typeof lil.bootLanguages === "function") {
    lil.bootLanguages();
  }
  const KeyMod = {
    CtrlCmd: lil.KeyModCtrl ?? 2048,
    Shift: lil.KeyModShift ?? 1024,
    Alt: lil.KeyModAlt ?? 512,
    WinCtrl: lil.KeyModWinCtrl ?? 256,
    chord(first, second) {
      return first | second;
    }
  };
  const MarkerSeverity = {
    Hint: lil.MarkerSeverityHint ?? 1,
    Info: lil.MarkerSeverityInfo ?? 2,
    Warning: lil.MarkerSeverityWarning ?? 4,
    Error: lil.MarkerSeverityError ?? 8
  };
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
    Snippet: 27
  };
  function Position(lineNumber, column) {
    return lil.Position ? lil.Position(lineNumber, column) : { lineNumber, column };
  }
  function Range(startLineNumber, startColumn, endLineNumber, endColumn) {
    return lil.Range ? lil.Range(startLineNumber, startColumn, endLineNumber, endColumn) : { startLineNumber, startColumn, endLineNumber, endColumn };
  }
  function Selection(selectionStartLineNumber, selectionStartColumn, positionLineNumber, positionColumn) {
    return lil.Selection ? lil.Selection(selectionStartLineNumber, selectionStartColumn, positionLineNumber, positionColumn) : { selectionStartLineNumber, selectionStartColumn, positionLineNumber, positionColumn };
  }
  function unwrap(value) {
    return value && value._handle ? value._handle : value;
  }
  function disposable(fn) {
    return { dispose: typeof fn === "function" ? fn : () => {
    } };
  }
  function eventHub() {
    const fns = [];
    const event = (listener) => {
      fns.push(listener);
      return disposable(() => {
        const i2 = fns.indexOf(listener);
        if (i2 >= 0) {
          fns.splice(i2, 1);
        }
      });
    };
    event._fire = (payload) => {
      for (const fn of fns.slice()) {
        fn(payload);
      }
    };
    return event;
  }
  function packSels(sels) {
    const packed = [];
    for (const sel of sels ?? []) {
      packed.push(
        sel.selectionStartLineNumber ?? sel.startLineNumber ?? 1,
        sel.selectionStartColumn ?? sel.startColumn ?? 1,
        sel.positionLineNumber ?? sel.endLineNumber ?? 1,
        sel.positionColumn ?? sel.endColumn ?? 1
      );
    }
    return packed;
  }
  function unpackSels(packed) {
    const out = [];
    for (let i2 = 0; i2 + 3 < (packed ?? []).length; i2 += 4) {
      out.push(Selection(packed[i2], packed[i2 + 1], packed[i2 + 2], packed[i2 + 3]));
    }
    return out;
  }
  function escapeHtml(text) {
    return String(text ?? "").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }
  const modelCreated = eventHub();
  const modelDisposed = eventHub();
  const modelLanguage = eventHub();
  const markersChanged = eventHub();
  const languageEncountered = /* @__PURE__ */ new Map();
  const globalCommands = [];
  const globalActions = [];
  const keybindingRules = [];
  const linkOpeners = [];
  const editorOpeners = [];
  let colorMap = null;
  let editorSeq = 0;
  class CancellationTokenSource {
    constructor() {
      this.token = {
        isCancellationRequested: false,
        onCancellationRequested: eventHub()
      };
    }
    cancel() {
      this.token.isCancellationRequested = true;
      this.token.onCancellationRequested._fire();
    }
    dispose() {
    }
  }
  class Emitter {
    constructor() {
      this.event = eventHub();
    }
    fire(value) {
      this.event._fire(value);
    }
    dispose() {
    }
  }
  function wrapModel(handle) {
    if (!handle) {
      return null;
    }
    if (handle._handle && handle.getValue) {
      return handle;
    }
    const uriString = typeof lil.modelUriString === "function" ? lil.modelUriString(handle) : "inmemory://model";
    const uri = {
      scheme: "inmemory",
      authority: "",
      path: uriString,
      query: "",
      fragment: "",
      fsPath: uriString,
      toString() {
        return uriString;
      }
    };
    const model = {
      _handle: handle,
      uri,
      getValue() {
        return lil.modelGetValue ? lil.modelGetValue(handle) : "";
      },
      setValue(value) {
        lil.modelSetValue?.(handle, value ?? "");
      },
      getLineCount() {
        return lil.modelGetLineCount ? lil.modelGetLineCount(handle) : 1;
      },
      getLineContent(line) {
        return lil.modelGetLineContent ? lil.modelGetLineContent(handle, line) : "";
      },
      getLineLength(line) {
        return (model.getLineContent(line) ?? "").length;
      },
      getLanguageId() {
        return lil.modelGetLanguageId ? lil.modelGetLanguageId(handle) : "plaintext";
      },
      getVersionId() {
        return lil.modelGetVersionId ? lil.modelGetVersionId(handle) : 1;
      },
      getOffsetAt(pos) {
        return lil.modelGetOffsetAt ? lil.modelGetOffsetAt(handle, pos) : 0;
      },
      getPositionAt(offset) {
        return lil.modelGetPositionAt ? lil.modelGetPositionAt(handle, offset) : Position(1, 1);
      },
      getValueInRange(range) {
        return lil.modelGetValueInRange ? lil.modelGetValueInRange(handle, range) : "";
      },
      getWordAtPosition(pos) {
        const row = lil.modelGetWordAtPosition ? lil.modelGetWordAtPosition(handle, pos) : null;
        if (!row || row.length < 3 || !row[2]) {
          return null;
        }
        return { word: row[2], startColumn: Number(row[0]), endColumn: Number(row[1]) };
      },
      getFullModelRange() {
        const last = model.getLineCount();
        const col = (model.getLineContent(last) ?? "").length + 1;
        return Range(1, 1, last, col);
      },
      getEOL() {
        return "\n";
      },
      findMatches(query, _searchScope, isRegex, matchCase, wholeWord, _capture, limit) {
        if (lil.modelFindMatches) {
          return lil.modelFindMatches(handle, query ?? "", !!isRegex, matchCase !== false, !!wholeWord, limit ?? 1e3);
        }
        return [];
      },
      applyEdits(edits) {
        const ops = (edits ?? []).map(
          (e) => lil.editOp(
            e.range.startLineNumber,
            e.range.startColumn,
            e.range.endLineNumber,
            e.range.endColumn,
            e.text ?? ""
          )
        );
        return lil.modelApplyEdits ? lil.modelApplyEdits(handle, ops) : [];
      },
      pushEditOperations(_before, edits) {
        return model.applyEdits(edits);
      },
      deltaDecorations(oldIds, next) {
        if (!lil.modelDeltaDecorations || !lil.deco) {
          return [];
        }
        const decos = (next ?? []).map(
          (d2) => lil.deco(
            d2.range.startLineNumber,
            d2.range.startColumn,
            d2.range.endLineNumber,
            d2.range.endColumn,
            d2.options?.inlineClassName ?? d2.options?.className ?? d2.options?.glyphMarginClassName ?? ""
          )
        );
        return lil.modelDeltaDecorations(handle, oldIds ?? [], decos);
      },
      undo() {
        return lil.modelUndo ? lil.modelUndo(handle) : false;
      },
      redo() {
        return lil.modelRedo ? lil.modelRedo(handle) : false;
      },
      dispose() {
        lil.modelDispose?.(handle);
      },
      onDidChangeContent(listener) {
        if (typeof lil.modelOnDidChangeContent === "function") {
          const dispose = lil.modelOnDidChangeContent(handle, () => listener({ changes: [], eol: "\n", versionId: model.getVersionId() }));
          return { dispose };
        }
        return { dispose() {
        } };
      },
      getLinesContent() {
        return lil.modelGetLinesContent ? lil.modelGetLinesContent(handle) : [model.getValue()];
      },
      getValueLength() {
        return lil.modelGetValueLength ? lil.modelGetValueLength(handle) : model.getValue().length;
      },
      getLineMaxColumn(line) {
        return model.getLineLength(line) + 1;
      },
      getLineMinColumn() {
        return 1;
      },
      getLineFirstNonWhitespaceColumn(line) {
        const text = model.getLineContent(line);
        const m2 = text.match(/\S/);
        return m2 ? m2.index + 1 : 0;
      },
      getLineLastNonWhitespaceColumn(line) {
        const text = model.getLineContent(line);
        const m2 = text.match(/\S\s*$/);
        return m2 ? m2.index + 2 : 0;
      },
      pushStackElement() {
        lil.modelPushStackElement?.(handle);
      },
      pushUndoStop() {
        lil.modelPushStackElement?.(handle);
        return true;
      },
      popUndoStop() {
        return false;
      },
      findNextMatch(query, start, isRegex, matchCase, wholeWord) {
        const packed = lil.modelFindNextMatchPacked ? lil.modelFindNextMatchPacked(handle, query ?? "", start?.lineNumber ?? 1, start?.column ?? 1, !!isRegex, matchCase !== false, !!wholeWord) : [];
        if (!packed || packed.length < 4) {
          return null;
        }
        return { range: Range(packed[0], packed[1], packed[2], packed[3]), matches: [query] };
      },
      findPreviousMatch(query, start, isRegex, matchCase, wholeWord) {
        const hits = model.findMatches(query, false, isRegex, matchCase, wholeWord, false, 1e3);
        let last = null;
        for (const hit of hits) {
          const r2 = hit.range ?? hit;
          if (r2.startLineNumber < start.lineNumber || r2.startLineNumber === start.lineNumber && r2.startColumn < start.column) {
            last = hit;
          }
        }
        return last;
      },
      getWordUntilPosition(pos) {
        const word = model.getWordAtPosition(pos);
        if (!word) {
          return { word: "", startColumn: pos.column, endColumn: pos.column };
        }
        return { word: word.word.slice(0, Math.max(0, pos.column - word.startColumn)), startColumn: word.startColumn, endColumn: pos.column };
      },
      validatePosition(pos) {
        const last = model.getLineCount();
        let line = pos.lineNumber | 0;
        if (line < 1) line = 1;
        if (line > last) line = last;
        let col = pos.column | 0;
        const max = model.getLineMaxColumn(line);
        if (col < 1) col = 1;
        if (col > max) col = max;
        return Position(line, col);
      },
      validateRange(range) {
        const s2 = model.validatePosition({ lineNumber: range.startLineNumber, column: range.startColumn });
        const e = model.validatePosition({ lineNumber: range.endLineNumber, column: range.endColumn });
        return Range(s2.lineNumber, s2.column, e.lineNumber, e.column);
      },
      modifyPosition(pos, offset) {
        return model.getPositionAt(model.getOffsetAt(pos) + (offset | 0));
      },
      getOptions() {
        return { tabSize: 4, insertSpaces: true, defaultEOL: 1, trimAutoWhitespace: true, indentSize: 4, bracketPairColorizationOptions: { enabled: false } };
      },
      getFormattingOptions() {
        return { tabSize: 4, insertSpaces: true };
      },
      detectIndentation() {
      },
      normalizeIndentation(text) {
        return text;
      },
      updateOptions() {
      },
      isDisposed() {
        return false;
      },
      isAttachedToEditor() {
        return true;
      },
      getAlternativeVersionId() {
        return model.getVersionId();
      },
      setEOL() {
      },
      getEndOfLineSequence() {
        return 0;
      },
      onDidChangeLanguage(listener) {
        return modelLanguage((e) => {
          if (e.model === model) listener(e);
        });
      },
      onWillDispose(listener) {
        return modelDisposed((m2) => {
          if (m2 === model) listener();
        });
      },
      getAllDecorations() {
        return [];
      },
      getDecorationsInRange() {
        return [];
      },
      getLineDecorations() {
        return [];
      },
      getOverviewRulerDecorations() {
        return [];
      },
      getInjectedTextDecorations() {
        return [];
      },
      changeDecorations(cb2) {
        const acc = {
          addDecoration(range, options) {
            const ids = model.deltaDecorations([], [{ range, options }]);
            return ids[0];
          },
          changeDecoration() {
          },
          changeDecorationOptions() {
          },
          removeDecoration(id2) {
            model.deltaDecorations([id2], []);
          },
          deltaDecorations(oldIds, next) {
            return model.deltaDecorations(oldIds, next);
          }
        };
        return cb2(acc);
      }
    };
    return model;
  }
  function wrapEditor(handle) {
    const listeners = {
      content: [],
      cursor: []
    };
    if (typeof lil.editorOnDidChangeModelContent === "function") {
      lil.editorOnDidChangeModelContent(handle, () => {
        for (const fn of listeners.content) {
          fn({});
        }
      });
    }
    if (typeof lil.editorOnDidChangeCursorPosition === "function") {
      lil.editorOnDidChangeCursorPosition(handle, () => {
        for (const fn of listeners.cursor) {
          fn({ position: lil.editorGetPosition(handle) });
        }
      });
    }
    const wrapped = {
      _handle: handle,
      getValue() {
        return lil.editorGetValue(handle);
      },
      setValue(value) {
        lil.editorSetValue(handle, value ?? "");
      },
      getModel() {
        return wrapModel(lil.editorGetModel(handle));
      },
      setModel(model) {
        lil.editorSetModel(handle, unwrap(model));
        if (typeof lil.editorSetModelFacade === "function") {
          lil.editorSetModelFacade(handle, wrapModel(unwrap(model)));
        }
      },
      getPosition() {
        return lil.editorGetPosition(handle);
      },
      setPosition(pos) {
        lil.editorSetPosition(handle, pos);
      },
      getSelection() {
        return lil.editorGetSelection(handle);
      },
      setSelection(sel) {
        lil.editorSetSelection(handle, sel);
      },
      trigger(source, handlerId, payload) {
        lil.editorTrigger(handle, source, handlerId, payload ?? {});
      },
      layout(dimension) {
        if (dimension && typeof lil.editorLayoutSize === "function") {
          lil.editorLayoutSize(handle, dimension.width | 0, dimension.height | 0);
          return;
        }
        lil.editorLayout(handle);
      },
      focus() {
        lil.editorFocus(handle);
      },
      hasTextFocus() {
        return true;
      },
      dispose() {
        lil.editorDispose(handle);
      },
      executeEdits(_source, edits) {
        const ops = (edits ?? []).map(
          (e) => lil.editOp(
            e.range.startLineNumber,
            e.range.startColumn,
            e.range.endLineNumber,
            e.range.endColumn,
            e.text ?? ""
          )
        );
        return lil.editorExecuteEdits(handle, ops);
      },
      deltaDecorations(oldIds, next) {
        const decos = (next ?? []).map(
          (d2) => lil.deco(
            d2.range.startLineNumber,
            d2.range.startColumn,
            d2.range.endLineNumber,
            d2.range.endColumn,
            d2.options?.inlineClassName ?? d2.options?.className ?? ""
          )
        );
        return lil.editorDeltaDecorations(handle, oldIds ?? [], decos);
      },
      revealLine(line) {
        lil.editorRevealLine(handle, line);
      },
      revealLineInCenter(line) {
        lil.editorRevealLine(handle, line);
      },
      revealPosition(pos) {
        lil.editorSetPosition(handle, pos);
      },
      revealRange(range) {
        lil.editorRevealLine(handle, range.startLineNumber);
      },
      addAction(desc) {
        lil.editorAddAction(handle, desc.id, desc.label ?? desc.id, desc.run);
        return {
          dispose() {
          }
        };
      },
      addCommand(_keybinding, handler) {
        const id2 = "cmd." + Math.random().toString(36).slice(2);
        lil.editorAddAction(handle, id2, id2, handler);
        return id2;
      },
      getAction(id2) {
        return {
          id: id2,
          run() {
            lil.editorTrigger(handle, "action", id2, {});
          }
        };
      },
      updateOptions(options) {
        lil.editorUpdateOptions?.(handle, options ?? {});
      },
      getOptions() {
        return {
          get() {
            return void 0;
          }
        };
      },
      saveViewState() {
        const pos = lil.editorGetPosition(handle);
        return { cursorState: [{ inSelectionMode: false, position: pos }] };
      },
      restoreViewState(state) {
        const pos = state?.cursorState?.[0]?.position;
        if (pos) {
          lil.editorSetPosition(handle, pos);
        }
      },
      onDidChangeModelContent(listener) {
        listeners.content.push(listener);
        return {
          dispose() {
            listeners.content = listeners.content.filter((fn) => fn !== listener);
          }
        };
      },
      onDidChangeCursorPosition(listener) {
        listeners.cursor.push(listener);
        return {
          dispose() {
            listeners.cursor = listeners.cursor.filter((fn) => fn !== listener);
          }
        };
      },
      onDidChangeModel(listener) {
        return { dispose() {
        } };
      },
      onDidFocusEditorText(listener) {
        return { dispose() {
        } };
      },
      onDidBlurEditorText(listener) {
        return { dispose() {
        } };
      },
      onDidChangeCursorSelection(listener) {
        return wrapped.onDidChangeCursorPosition((e) => listener({ selection: wrapped.getSelection(), ...e }));
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
          editorSeq += 1;
          wrapped._id = "code-" + editorSeq;
        }
        return wrapped._id;
      },
      getEditorType() {
        return "vs.editor.ICodeEditor";
      },
      getContainerDomNode() {
        return lil.editorGetDomNode ? lil.editorGetDomNode(handle) : null;
      },
      getDomNode() {
        return wrapped.getContainerDomNode();
      },
      getOverflowWidgetsDomNode() {
        return lil.editorGetOverflowWidgetsDomNode ? lil.editorGetOverflowWidgetsDomNode(handle) : null;
      },
      getScrollTop() {
        return lil.editorGetScrollTop ? lil.editorGetScrollTop(handle) : 0;
      },
      setScrollTop(value) {
        lil.editorSetScrollTop?.(handle, value | 0);
      },
      getScrollLeft() {
        return lil.editorGetScrollLeft ? lil.editorGetScrollLeft(handle) : 0;
      },
      setScrollLeft(value) {
        lil.editorSetScrollLeft?.(handle, value | 0);
      },
      setScrollPosition(pos) {
        if (pos && pos.scrollTop != null) wrapped.setScrollTop(pos.scrollTop);
        if (pos && pos.scrollLeft != null) wrapped.setScrollLeft(pos.scrollLeft);
      },
      hasPendingScrollAnimation() {
        return false;
      },
      getContentHeight() {
        return lil.editorGetContentHeight ? lil.editorGetContentHeight(handle) : 0;
      },
      getContentWidth() {
        return lil.editorGetContentWidth ? lil.editorGetContentWidth(handle) : 0;
      },
      getScrollHeight() {
        return wrapped.getContentHeight();
      },
      getScrollWidth() {
        return wrapped.getContentWidth();
      },
      getVisibleRanges() {
        const packed = lil.editorGetVisibleRangePacked ? lil.editorGetVisibleRangePacked(handle) : [1, 1, 1, 1];
        return [Range(packed[0], packed[1], packed[2], packed[3])];
      },
      getSelections() {
        if (lil.editorGetSelectionsPacked) {
          const sels = unpackSels(lil.editorGetSelectionsPacked(handle));
          return sels.length ? sels : [wrapped.getSelection()];
        }
        return [wrapped.getSelection()];
      },
      setSelections(sels) {
        if (lil.editorSetSelectionsPacked) {
          lil.editorSetSelectionsPacked(handle, packSels(sels));
          return;
        }
        if (sels && sels[0]) {
          wrapped.setSelection(sels[0]);
        }
      },
      getTopForLineNumber(line) {
        return ((line | 0) - 1) * 19;
      },
      getBottomForLineNumber(line) {
        return (line | 0) * 19;
      },
      getTopForPosition(line) {
        return wrapped.getTopForLineNumber(line);
      },
      getLineHeightForPosition() {
        return 19;
      },
      getOffsetForColumn(_line, column) {
        return ((column | 0) - 1) * 8;
      },
      getScrolledVisiblePosition(pos) {
        return { top: wrapped.getTopForLineNumber(pos.lineNumber) - wrapped.getScrollTop(), left: wrapped.getOffsetForColumn(pos.lineNumber, pos.column), height: 19 };
      },
      getTargetAtClientPoint() {
        return null;
      },
      createDecorationsCollection(decos) {
        let ids = wrapped.deltaDecorations([], decos ?? []);
        return {
          set(next) {
            ids = wrapped.deltaDecorations(ids, next ?? []);
          },
          clear() {
            ids = wrapped.deltaDecorations(ids, []);
          },
          getRanges() {
            return [];
          },
          has() {
            return ids.length > 0;
          },
          length: ids.length
        };
      },
      removeDecorations(ids) {
        wrapped.deltaDecorations(ids ?? [], []);
      },
      getLineDecorations() {
        return [];
      },
      getDecorationsInRange() {
        return [];
      },
      createContextKey(key, value) {
        let current = value;
        return {
          set(next) {
            current = next;
          },
          get() {
            return current;
          },
          reset() {
            current = value;
          }
        };
      },
      addContentWidget(widget) {
        const host = wrapped.getOverflowWidgetsDomNode();
        const node = widget?.getDomNode?.();
        if (host && node && !node.parentNode) {
          host.appendChild(node);
        }
      },
      layoutContentWidget() {
      },
      removeContentWidget(widget) {
        widget?.getDomNode?.()?.remove?.();
      },
      addOverlayWidget(widget) {
        wrapped.addContentWidget(widget);
      },
      layoutOverlayWidget() {
      },
      removeOverlayWidget(widget) {
        wrapped.removeContentWidget(widget);
      },
      addGlyphMarginWidget() {
      },
      layoutGlyphMarginWidget() {
      },
      removeGlyphMarginWidget() {
      },
      changeViewZones() {
      },
      render() {
        lil.editorLayout?.(handle);
      },
      renderAsync() {
        wrapped.render();
      },
      applyFontInfo() {
      },
      setBanner() {
      },
      writeScreenReaderContent() {
      },
      hasWidgetFocus() {
        return true;
      },
      getContribution() {
        return null;
      },
      getSupportedActions() {
        return [
          "undo",
          "redo",
          "actions.find",
          "editor.action.commentLine",
          "editor.action.triggerSuggest",
          "editor.foldAll",
          "editor.unfoldAll",
          "editor.action.formatDocument"
        ].map((id2) => wrapped.getAction(id2));
      },
      executeCommand(_source, command) {
        if (command && typeof command.getEditOperations === "function") {
          return;
        }
      },
      executeCommands() {
      },
      pushUndoStop() {
        const model = wrapped.getModel();
        model?.pushUndoStop?.();
        return true;
      },
      popUndoStop() {
        return false;
      },
      revealAllCursors() {
        const pos = wrapped.getPosition();
        if (pos) wrapped.revealLine(pos.lineNumber);
      },
      revealLineInCenterIfOutsideViewport(line) {
        wrapped.revealLine(line);
      },
      revealPositionInCenter(pos) {
        wrapped.revealPosition(pos);
      },
      revealRangeInCenter(range) {
        wrapped.revealRange(range);
      },
      revealRangeAtTop(range) {
        wrapped.revealRange(range);
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
          minimap: { renderMinimap: 0, minimapLeft: 0, minimapWidth: 0 }
        };
      },
      getRawOptions() {
        return {};
      },
      getOption() {
        return void 0;
      },
      getConfiguredWordAtPosition(pos) {
        return wrapped.getModel()?.getWordAtPosition(pos) ?? null;
      },
      getFontSizeAtPosition() {
        return "14px";
      }
    };
    if (typeof lil.editorSetModelFacade === "function") {
      lil.editorSetModelFacade(handle, wrapped.getModel());
    }
    for (const action of globalActions) {
      wrapped.addAction(action);
    }
    return wrapped;
  }
  function wrapDiff(handle) {
    const original = wrapEditor(lil.diffGetOriginal ? lil.diffGetOriginal(handle) : handle);
    const modified = wrapEditor(lil.diffGetModified ? lil.diffGetModified(handle) : handle);
    return {
      _handle: handle,
      getOriginalEditor() {
        return original;
      },
      getModifiedEditor() {
        return modified;
      },
      getLineChanges() {
        return typeof lil.diffLineChanges === "function" ? lil.diffLineChanges(handle) : [];
      },
      getDiffLineInformationForOriginal() {
        return null;
      },
      getDiffLineInformationForModified() {
        return null;
      },
      setModel(model) {
        if (model?.original) {
          original.setModel(model.original);
        }
        if (model?.modified) {
          modified.setModel(model.modified);
        }
      },
      getModel() {
        return { original: original.getModel(), modified: modified.getModel() };
      },
      layout() {
        lil.diffLayout?.(handle);
        original.layout();
        modified.layout();
      },
      updateOptions() {
      },
      onDidUpdateDiff: eventHub(),
      onDidChangeModel: eventHub(),
      getContainerDomNode() {
        return original.getContainerDomNode();
      },
      getEditorType() {
        return "vs.editor.IDiffEditor";
      },
      addCommand(keybinding, handler) {
        return original.addCommand(keybinding, handler);
      },
      createContextKey(key, value) {
        return original.createContextKey(key, value);
      },
      addAction(desc) {
        original.addAction(desc);
        return modified.addAction(desc);
      },
      dispose() {
        lil.diffDispose(handle);
      }
    };
  }
  const editor = {
    create(dom, options = {}) {
      const next = { ...options };
      const existing = next.model ? unwrap(next.model) : null;
      if (existing) {
        next.value = next.value ?? (lil.modelGetValue ? lil.modelGetValue(existing) : "");
        next.language = next.language ?? (lil.modelGetLanguageId ? lil.modelGetLanguageId(existing) : "plaintext");
      }
      const handle = lil.create(dom, next);
      const wrapped = wrapEditor(handle);
      if (existing) {
        wrapped.setModel(wrapModel(existing));
      }
      return wrapped;
    },
    createDiffEditor(dom, options) {
      return wrapDiff(lil.createDiffEditor(dom, options ?? {}));
    },
    createModel(value, language, uri) {
      const wrapped = uri && lil.createModelWithUri ? wrapModel(lil.createModelWithUri(value ?? "", language ?? "plaintext", uri)) : wrapModel(lil.createModel(value ?? "", language ?? "plaintext"));
      modelCreated._fire(wrapped);
      return wrapped;
    },
    setTheme(name) {
      lil.setTheme(name);
    },
    defineTheme(name, data) {
      lil.defineTheme(name, data);
    },
    setModelLanguage(model, languageId) {
      const oldLanguage = model?.getLanguageId?.() ?? "";
      lil.setModelLanguage(unwrap(model), languageId);
      modelLanguage._fire({ model, oldLanguage });
    },
    setModelMarkers(model, owner, markers) {
      lil.setModelMarkers(unwrap(model), owner, markers ?? []);
      markersChanged._fire([model?.uri].filter(Boolean));
    },
    getModelMarkers(filter = {}) {
      return lil.getModelMarkers(filter.owner ?? "", filter.resource ?? "", filter.take ?? 0);
    },
    removeAllMarkers(owner) {
      lil.removeAllMarkers(owner);
    },
    getModels() {
      return (lil.getModels?.() ?? []).map(wrapModel);
    },
    getModel(uri) {
      const models = editor.getModels();
      const key = uri?.toString?.() ?? String(uri ?? "");
      return models.find((m2) => (m2.uri?.toString?.() ?? "") === key) ?? null;
    },
    getEditors() {
      return (lil.getEditors?.() ?? []).map(wrapEditor);
    },
    getDiffEditors() {
      return (lil.getDiffEditors?.() ?? []).map(wrapDiff);
    },
    onDidCreateEditor(listener) {
      if (typeof lil.onDidCreateEditor === "function") {
        return disposable(lil.onDidCreateEditor(() => listener({})));
      }
      return disposable();
    },
    onDidCreateDiffEditor(listener) {
      if (typeof lil.onDidCreateDiffEditor === "function") {
        return disposable(lil.onDidCreateDiffEditor(() => listener({})));
      }
      return disposable();
    },
    createMultiFileDiffEditor(dom, override) {
      return editor.createDiffEditor(dom, override);
    },
    addCommand(descriptor) {
      globalCommands.push(descriptor);
      return disposable(() => {
        const i2 = globalCommands.indexOf(descriptor);
        if (i2 >= 0) globalCommands.splice(i2, 1);
      });
    },
    addEditorAction(descriptor) {
      globalActions.push(descriptor);
      for (const ed2 of editor.getEditors()) {
        ed2.addAction(descriptor);
      }
      return disposable(() => {
        const i2 = globalActions.indexOf(descriptor);
        if (i2 >= 0) globalActions.splice(i2, 1);
      });
    },
    addKeybindingRule(rule) {
      keybindingRules.push(rule);
      return disposable();
    },
    addKeybindingRules(rules) {
      for (const rule of rules ?? []) {
        editor.addKeybindingRule(rule);
      }
      return disposable();
    },
    onDidChangeMarkers(listener) {
      return markersChanged(listener);
    },
    onDidCreateModel(listener) {
      return modelCreated(listener);
    },
    onWillDisposeModel(listener) {
      return modelDisposed(listener);
    },
    onDidChangeModelLanguage(listener) {
      return modelLanguage(listener);
    },
    createWebWorker() {
      return {
        getProxy() {
          return Promise.resolve({});
        },
        withSyncedResources() {
          return Promise.resolve({});
        },
        dispose() {
        }
      };
    },
    colorize(text, languageId) {
      const rows = editor.tokenize(text, languageId);
      const lines = String(text ?? "").split(/\r\n|\r|\n/);
      const html = lines.map((line, i2) => {
        const tokens = rows[i2] ?? [];
        if (!tokens.length) {
          return escapeHtml(line);
        }
        let out = "";
        let last = 0;
        for (const token of tokens) {
          const start = token.offset ?? token.startIndex ?? last;
          if (start > last) {
            out += escapeHtml(line.slice(last, start));
          }
          const cls = String(token.type ?? token.scopes ?? "source").replace(/\./g, " ");
          const end = tokens[tokens.indexOf(token) + 1]?.offset ?? line.length;
          out += `<span class="mtk ${cls}">${escapeHtml(line.slice(start, end))}</span>`;
          last = end;
        }
        if (last < line.length) {
          out += escapeHtml(line.slice(last));
        }
        return out;
      }).join("<br/>");
      return Promise.resolve(html);
    },
    colorizeElement(domNode, options) {
      const lang = domNode?.getAttribute?.("data-lang") ?? options?.theme ?? "plaintext";
      return editor.colorize(domNode?.textContent ?? "", lang, options ?? {}).then((html) => {
        if (domNode) domNode.innerHTML = html;
      });
    },
    colorizeModelLine(model, lineNumber) {
      const line = model.getLineContent(lineNumber);
      const rows = editor.tokenize(line, model.getLanguageId());
      return rows[0] ? rows[0].map((t2) => t2.type).join(" ") : line;
    },
    tokenize(text, languageId) {
      const packed = typeof lil.tokenizePacked === "function" ? lil.tokenizePacked(languageId, text) : [];
      const lines = String(text ?? "").split(/\r\n|\r|\n/);
      const rows = lines.map(() => []);
      let lineStart = 0;
      let line = 0;
      for (const row of packed) {
        const sep = String(row).indexOf(":");
        const offset = Number(String(row).slice(0, sep));
        const type = String(row).slice(sep + 1);
        while (line < lines.length - 1 && offset >= lineStart + lines[line].length + 1) {
          lineStart += lines[line].length + 1;
          line += 1;
        }
        rows[line].push({ offset: offset - lineStart, type, language: languageId });
      }
      return rows;
    },
    remeasureFonts() {
    },
    registerCommand(id2, handler) {
      return editor.addCommand({ id: id2, run: handler });
    },
    registerLinkOpener(opener) {
      linkOpeners.push(opener);
      return disposable(() => {
        const i2 = linkOpeners.indexOf(opener);
        if (i2 >= 0) linkOpeners.splice(i2, 1);
      });
    },
    registerEditorOpener(opener) {
      editorOpeners.push(opener);
      return disposable(() => {
        const i2 = editorOpeners.indexOf(opener);
        if (i2 >= 0) editorOpeners.splice(i2, 1);
      });
    },
    EditorType: {
      ICodeEditor: "vs.editor.ICodeEditor",
      IDiffEditor: "vs.editor.IDiffEditor"
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
    AccessibilitySupport: { Unknown: 0, Disabled: 1, Enabled: 2 }
  };
  const languages = {
    register(lang) {
      if (lang?.id && typeof lil.registerLanguageId === "function") {
        lil.registerLanguageId(lang.id);
        const pending = languageEncountered.get(lang.id);
        if (pending) {
          for (const fn of pending) fn();
          languageEncountered.delete(lang.id);
        }
      }
    },
    getLanguages() {
      return (lil.languageIds?.() ?? []).map((id2) => ({ id: id2 }));
    },
    getEncodedLanguageId(languageId) {
      let hash = 0;
      for (let i2 = 0; i2 < String(languageId ?? "").length; i2++) {
        hash = hash + languageId.charCodeAt(i2) & 255;
      }
      return hash || 1;
    },
    onLanguage(languageId, callback) {
      const ids = lil.languageIds?.() ?? [];
      if (ids.includes(languageId)) {
        callback();
        return disposable();
      }
      const list = languageEncountered.get(languageId) ?? [];
      list.push(callback);
      languageEncountered.set(languageId, list);
      return disposable(() => {
        const next = (languageEncountered.get(languageId) ?? []).filter((fn) => fn !== callback);
        languageEncountered.set(languageId, next);
      });
    },
    onLanguageEncountered(languageId, callback) {
      return languages.onLanguage(languageId, callback);
    },
    registerCompletionItemProvider(selector, provider) {
      return { dispose: lil.languagesRegisterCompletion(selector, provider) };
    },
    registerHoverProvider(selector, provider) {
      return { dispose: lil.languagesRegisterHover(selector, provider) };
    },
    registerDefinitionProvider(selector, provider) {
      return { dispose: lil.languagesRegisterDefinition(selector, provider) };
    },
    registerReferenceProvider(selector, provider) {
      return { dispose: lil.languagesRegisterReference(selector, provider) };
    },
    registerDocumentSymbolProvider(selector, provider) {
      return { dispose: lil.languagesRegisterDocumentSymbol(selector, provider) };
    },
    registerDocumentFormattingEditProvider(selector, provider) {
      return { dispose: lil.languagesRegisterFormatting(selector, provider) };
    },
    registerDocumentRangeFormattingEditProvider(selector, provider) {
      return { dispose: lil.languagesRegisterFormatting(selector, provider) };
    },
    registerRenameProvider(selector, provider) {
      return { dispose: lil.languagesRegisterRename(selector, provider) };
    },
    registerSignatureHelpProvider(selector, provider) {
      return { dispose: lil.languagesRegisterSignatureHelp(selector, provider) };
    },
    registerFoldingRangeProvider(selector, provider) {
      return { dispose: lil.languagesRegisterFolding(selector, provider) };
    },
    registerLinkProvider(selector, provider) {
      return { dispose: lil.languagesRegisterLink(selector, provider) };
    },
    registerCodeActionProvider(selector, provider) {
      return { dispose: lil.languagesRegisterCodeAction(selector, provider) };
    },
    registerCodeLensProvider(selector, provider) {
      return { dispose: lil.languagesRegisterCodeLens(selector, provider) };
    },
    registerColorProvider(selector, provider) {
      return { dispose: lil.languagesRegisterColor(selector, provider) };
    },
    registerDocumentHighlightProvider(selector, provider) {
      return { dispose: lil.languagesRegisterHighlight(selector, provider) };
    },
    registerInlayHintsProvider(selector, provider) {
      return { dispose: lil.languagesRegisterInlayHints(selector, provider) };
    },
    registerInlineCompletionsProvider(selector, provider) {
      return { dispose: lil.languagesRegisterInlineCompletions(selector, provider) };
    },
    registerImplementationProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("implementation", selector, provider) : () => {
      } };
    },
    registerTypeDefinitionProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("typeDefinition", selector, provider) : () => {
      } };
    },
    registerDeclarationProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("declaration", selector, provider) : () => {
      } };
    },
    registerSelectionRangeProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("selectionRange", selector, provider) : () => {
      } };
    },
    registerLinkedEditingRangeProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("linkedEditing", selector, provider) : () => {
      } };
    },
    registerOnTypeFormattingEditProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("onTypeFormatting", selector, provider) : () => {
      } };
    },
    registerDocumentSemanticTokensProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("documentSemanticTokens", selector, provider) : () => {
      } };
    },
    registerDocumentRangeSemanticTokensProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("documentRangeSemanticTokens", selector, provider) : () => {
      } };
    },
    registerNewSymbolNameProvider(selector, provider) {
      return { dispose: lil.languagesRegisterKind ? lil.languagesRegisterKind("newSymbolName", selector, provider) : () => {
      } };
    },
    registerTokensProviderFactory(languageId, factory) {
      Promise.resolve(factory?.create?.()).then((provider) => {
        if (provider) languages.setTokensProvider(languageId, provider);
      });
      return disposable();
    },
    setColorMap(map) {
      colorMap = map;
    },
    setLanguageConfiguration(id2, config) {
      lil.setLanguageConfigurationJs?.(id2, config);
      return { dispose() {
      } };
    },
    setMonarchTokensProvider(id2, def) {
      const provider = compileMonarch(id2, def);
      lil.setTokensProviderJs?.(id2, provider);
      return { dispose() {
      } };
    },
    setTokensProvider(id2, provider) {
      const tokenize = typeof provider?.tokenize === "function" ? (line, state, _c2) => {
        const out = provider.tokenize(line, state);
        return out;
      } : provider;
      lil.setTokensProviderJs?.(id2, typeof tokenize === "function" ? { tokenize } : provider);
      return { dispose() {
      } };
    },
    CompletionItemKind,
    CompletionItemInsertTextRule: { None: 0, KeepWhitespace: 1, InsertAsSnippet: 4 },
    CompletionItemTag: { Deprecated: 1 },
    CompletionTriggerKind: { Invoke: 0, TriggerCharacter: 1, TriggerForIncompleteCompletions: 2 },
    DocumentHighlightKind: { Text: 0, Read: 1, Write: 2 },
    SymbolKind: {
      File: 0,
      Module: 1,
      Namespace: 2,
      Package: 3,
      Class: 4,
      Method: 5,
      Property: 6,
      Field: 7,
      Constructor: 8,
      Enum: 9,
      Interface: 10,
      Function: 11,
      Variable: 12,
      Constant: 13,
      String: 14,
      Number: 15,
      Boolean: 16,
      Array: 17,
      Object: 18,
      Key: 19,
      Null: 20,
      EnumMember: 21,
      Struct: 22,
      Event: 23,
      Operator: 24,
      TypeParameter: 25
    },
    SymbolTag: { Deprecated: 1 },
    IndentAction: { None: 0, Indent: 1, IndentOutdent: 2, Outdent: 3 },
    FoldingRangeKind: { Comment: { value: "comment" }, Imports: { value: "imports" }, Region: { value: "region" } },
    SignatureHelpTriggerKind: { Invoke: 1, TriggerCharacter: 2, ContentChange: 3 },
    InlayHintKind: { Type: 1, Parameter: 2 },
    InlineCompletionTriggerKind: { Automatic: 0, Explicit: 1 },
    NewSymbolNameTriggerKind: { Invoke: 0, Automatic: 1 }
  };
  return {
    editor,
    languages,
    Range,
    Position,
    Selection,
    Uri: Object.assign(
      function Uri(scheme, authority, path, query, fragment) {
        const raw = `${scheme}://${authority ?? ""}${path ?? ""}${query ? "?" + query : ""}${fragment ? "#" + fragment : ""}`;
        return lil.parseUri ? lil.parseUri(raw) : { scheme, authority, path, query, fragment, toString: () => raw };
      },
      {
        parse(value) {
          return lil.parseUri ? lil.parseUri(String(value ?? "")) : { scheme: "", path: String(value ?? ""), toString: () => String(value ?? "") };
        },
        file(path) {
          return lil.fileUri ? lil.fileUri(String(path ?? "")) : { scheme: "file", path: String(path ?? ""), toString: () => "file://" + path };
        }
      }
    ),
    KeyCode: lil.KeyCode,
    KeyMod,
    MarkerSeverity,
    MarkerTag: { Unnecessary: 1, Deprecated: 2 },
    CompletionItemKind,
    CancellationTokenSource,
    Emitter,
    Token: function Token(offset, type, language) {
      this.offset = offset;
      this.type = type;
      this.language = language;
    },
    bindMonaco
  };
}

// build/monaco-layers/entry.raw.js
var entry_raw_exports = {};
__export(entry_raw_exports, {
  KeyModAlt: () => id,
  KeyModCtrl: () => gd,
  KeyModShift: () => hd,
  KeyModWinCtrl: () => jd,
  MarkerSeverityError: () => Wa,
  MarkerSeverityHint: () => fd,
  MarkerSeverityInfo: () => Mb,
  MarkerSeverityWarning: () => Nb,
  Position: () => _e,
  Range: () => $e,
  Selection: () => af,
  bootLanguages: () => Lb,
  computeIndentFolds: () => cf,
  computeLineDiff: () => mc,
  create: () => qd,
  createDiffEditor: () => rd,
  createModel: () => lc,
  createModelWithUri: () => md,
  deco: () => od,
  defineTheme: () => ud,
  diffDispose: () => Ge,
  diffGetModified: () => Ie,
  diffGetOriginal: () => He,
  diffLayout: () => Je,
  diffLineChanges: () => Fe,
  editOp: () => v,
  editorAddAction: () => fe,
  editorDeltaDecorations: () => de,
  editorDispose: () => ae,
  editorExecuteEdits: () => ce,
  editorFocus: () => be,
  editorGetContentHeight: () => Re,
  editorGetContentWidth: () => Se,
  editorGetDomNode: () => Le,
  editorGetModel: () => Td,
  editorGetOverflowWidgetsDomNode: () => Me,
  editorGetPosition: () => Vd,
  editorGetScrollLeft: () => Pe,
  editorGetScrollTop: () => Ne,
  editorGetSelection: () => Xd,
  editorGetSelectionsPacked: () => Ue,
  editorGetValue: () => Rd,
  editorGetVisibleRangePacked: () => Te,
  editorLayout: () => _d,
  editorLayoutSize: () => $d,
  editorOnDidChangeCursorPosition: () => ie,
  editorOnDidChangeModelContent: () => he,
  editorRevealLine: () => ee,
  editorSetModel: () => Ud,
  editorSetModelFacade: () => ge,
  editorSetPosition: () => Wd,
  editorSetScrollLeft: () => Qe,
  editorSetScrollTop: () => Oe,
  editorSetSelection: () => Yd,
  editorSetSelectionsPacked: () => Ve,
  editorSetValue: () => Sd,
  editorTrigger: () => Zd,
  editorUpdateOptions: () => me,
  fileUri: () => ld,
  findInEditor: () => bf,
  getDiffEditors: () => td,
  getEditors: () => sd,
  getModelMarkers: () => yd,
  getModels: () => nd,
  gotoLine: () => hf,
  hoverAt: () => ef,
  languageIds: () => pd,
  languagesRegisterCodeAction: () => Kd,
  languagesRegisterCodeLens: () => Ld,
  languagesRegisterColor: () => Md,
  languagesRegisterCompletion: () => Ad,
  languagesRegisterDefinition: () => Cd,
  languagesRegisterDocumentSymbol: () => Ed,
  languagesRegisterFolding: () => Id,
  languagesRegisterFormatting: () => Fd,
  languagesRegisterHighlight: () => Nd,
  languagesRegisterHover: () => Bd,
  languagesRegisterInlayHints: () => Od,
  languagesRegisterInlineCompletions: () => Pd,
  languagesRegisterKind: () => Qd,
  languagesRegisterLink: () => Jd,
  languagesRegisterReference: () => Dd,
  languagesRegisterRename: () => Gd,
  languagesRegisterSignatureHelp: () => Hd,
  matchBracket: () => df,
  modelApplyEdits: () => De,
  modelDeltaDecorations: () => ne,
  modelDispose: () => Ee,
  modelFindMatches: () => Ce,
  modelFindNextMatchPacked: () => Ze,
  modelGetLanguageId: () => ze,
  modelGetLineContent: () => we,
  modelGetLineCount: () => ve,
  modelGetLinesContent: () => We,
  modelGetOffsetAt: () => xe,
  modelGetPositionAt: () => ye,
  modelGetValue: () => te,
  modelGetValueInRange: () => re,
  modelGetValueLength: () => Xe,
  modelGetVersionId: () => Ae,
  modelGetWordAtPosition: () => se,
  modelOnDidChangeContent: () => qe,
  modelPushStackElement: () => Ye,
  modelRedo: () => pe,
  modelSetValue: () => ue,
  modelUndo: () => oe,
  modelUriString: () => Be,
  onDidCreateDiffEditor: () => Ke,
  onDidCreateEditor: () => je,
  parseUri: () => kd,
  registerLanguageId: () => dd,
  removeAllMarkers: () => zd,
  setLanguageConfigurationJs: () => ke,
  setModelLanguage: () => wd,
  setModelMarkers: () => xd,
  setTheme: () => vd,
  setTokensProviderJs: () => le,
  suggestAt: () => ff,
  toggleLineComment: () => gf,
  tokenize: () => jf,
  tokenizePacked: () => kf
});

// ports/monaco/js-host.ts
function setTextContent(el, text) {
  el.textContent = text;
}
function setInnerHTML(el, html) {
  el.innerHTML = html;
}
function setClassName(el, name) {
  el.className = name;
}
function setStyle(el, prop, value) {
  el.style.setProperty(prop, value);
}
function focusElement(el) {
  el.focus();
}
function preventDefault(event) {
  event.preventDefault();
}
function eventKey(event) {
  return event.key;
}
function eventCtrlKey(event) {
  return event.ctrlKey || event.metaKey;
}
function eventShiftKey(event) {
  return event.shiftKey;
}
function eventAltKey(event) {
  return event.altKey;
}
function inputSetValue(el, value) {
  el.value = value;
}
function inputGetValue(el) {
  return el.value;
}
function canvasGetContext2d(canvas) {
  try {
    return canvas.getContext("2d");
  } catch {
    return null;
  }
}
function canvasSetSize(canvas, width, height) {
  canvas.width = width;
  canvas.height = height;
}
function canvasFillRect(ctx, x2, y2, w2, h, color) {
  if (!ctx) {
    return;
  }
  ctx.fillStyle = color;
  ctx.fillRect(x2, y2, w2, h);
}
function setTabIndex(el, value) {
  el.tabIndex = value;
}
var memoryClipboard = "";
function clipboardRead() {
  return memoryClipboard;
}
function clipboardWrite(text) {
  memoryClipboard = text;
  try {
    void navigator.clipboard?.writeText(text);
  } catch {
  }
}
function clipboardReadEvent(event) {
  const data = event.clipboardData?.getData("text/plain");
  if (typeof data === "string" && data.length > 0) {
    memoryClipboard = data;
    return data;
  }
  return memoryClipboard;
}
function clipboardWriteEvent(event, text) {
  memoryClipboard = text;
  event.clipboardData?.setData("text/plain", text);
}
function eventClientX(event) {
  return event.clientX | 0;
}
function eventClientY(event) {
  return event.clientY | 0;
}
function eventDetail(event) {
  return event.detail | 0;
}
function rectLeft(el) {
  return el.getBoundingClientRect().left | 0;
}
function rectTop(el) {
  return el.getBoundingClientRect().top | 0;
}
function setPlaceholder(el, value) {
  el.placeholder = value;
}
function setDisplay(el, value) {
  el.style.display = value;
}
function hostCall(obj, name, a, b, c) {
  if (obj == null) {
    return void 0;
  }
  const fn = obj[name];
  if (typeof fn !== "function") {
    return void 0;
  }
  try {
    return fn.call(obj, a, b, c);
  } catch {
    return void 0;
  }
}
function jsArrayLen(value) {
  if (Array.isArray(value)) {
    return value.length;
  }
  if (value && Array.isArray(value.suggestions)) {
    return value.suggestions.length;
  }
  if (value && Array.isArray(value.items)) {
    return value.items.length;
  }
  if (value && Array.isArray(value.lenses)) {
    return value.lenses.length;
  }
  if (value && Array.isArray(value.symbols)) {
    return value.symbols.length;
  }
  if (value && Array.isArray(value.contents)) {
    return value.contents.length;
  }
  return 0;
}
function jsArrayAt(value, index) {
  if (Array.isArray(value)) {
    return value[index];
  }
  const arr = value?.suggestions ?? value?.items ?? value?.lenses ?? value?.symbols ?? value?.contents;
  return Array.isArray(arr) ? arr[index] : void 0;
}
function jsPropString(value, key, fallback = "") {
  if (value == null) {
    return fallback;
  }
  const v2 = value[key];
  if (v2 == null) {
    return fallback;
  }
  if (typeof v2 === "string") {
    return v2;
  }
  if (typeof v2 === "object" && typeof v2.value === "string") {
    return v2.value;
  }
  if (typeof v2 === "object" && typeof v2.label === "string") {
    return v2.label;
  }
  return String(v2);
}
function jsPropInt(value, key, fallback = 0) {
  if (value == null) {
    return fallback;
  }
  const v2 = value[key];
  return typeof v2 === "number" ? v2 | 0 : fallback;
}
function emptyBuf() {
  return Array(1).join("");
}
function beginIdList(holder) {
  holder._ids = [];
}
function pushId(holder, id2) {
  holder._ids.push(id2 | 0);
}
function takeIdList(holder) {
  const out = holder._ids ?? [];
  holder._ids = [];
  return out;
}
function concat2(a, b) {
  return a + b;
}
function pushNewBuffer(tree, text, lineStarts) {
  const buf = { buffer: text, lineStarts };
  tree.tmpBuffer = buf;
  tree.buffers.push(buf);
}
function makeNodePos(node, remainder, start) {
  const rem = remainder | 0;
  const off = start | 0;
  const pos = [node, rem, off];
  pos.node = node;
  pos.remainder = rem;
  pos.nodeStartOffset = off;
  return pos;
}
function hostNodeNext(node, sentinel) {
  if (node.right !== sentinel) {
    let cur2 = node.right;
    while (cur2.left !== sentinel) {
      cur2 = cur2.left;
    }
    return cur2;
  }
  let cur = node;
  let guard = 0;
  while (cur.parent !== sentinel && guard < 1e5) {
    guard++;
    if (cur.parent.left === cur) {
      break;
    }
    cur = cur.parent;
  }
  if (cur.parent === sentinel || guard >= 1e5) {
    return sentinel;
  }
  return cur.parent;
}
function hostAccumulatedValue(tree, node, index) {
  if (index < 0) {
    return 0;
  }
  const piece = node.piece;
  const lineStarts = tree.buffers[piece.bufferIndex].lineStarts;
  const expected = piece.start.line + index + 1 | 0;
  if (expected > piece.end.line) {
    return (lineStarts[piece.end.line] | 0) + piece.end.column - (lineStarts[piece.start.line] | 0) - piece.start.column | 0;
  }
  return (lineStarts[expected] | 0) - (lineStarts[piece.start.line] | 0) - piece.start.column | 0;
}
function hostOffsetOfNode(tree, node) {
  let pos = node.size_left | 0;
  let cur = node;
  while (cur !== tree.root) {
    if (cur.parent.right === cur) {
      pos = pos + (cur.parent.size_left | 0) + cur.parent.piece.length | 0;
    }
    cur = cur.parent;
  }
  return pos;
}
function hostNodeAt(tree, offset, sentinel) {
  offset = offset | 0;
  if (tree.cacheValid) {
    const cached = tree.cacheNode;
    const cStart = tree.cacheNodeStartOffset | 0;
    const cLen = cached.piece.length | 0;
    if (cStart <= offset && cStart + cLen >= offset) {
      return makeNodePos(cached, offset - cStart, cStart);
    }
  }
  let rest = offset;
  let x2 = tree.root;
  let nodeStartOffset = 0;
  let guard = 0;
  while (x2 !== sentinel && guard < 1e5) {
    const sizeLeft = x2.size_left | 0;
    const pieceLen = x2.piece.length | 0;
    if (sizeLeft > rest) {
      x2 = x2.left;
    } else if (sizeLeft + pieceLen >= rest) {
      nodeStartOffset = nodeStartOffset + sizeLeft | 0;
      tree.cacheNode = x2;
      tree.cacheNodeStartOffset = nodeStartOffset;
      tree.cacheNodeStartLineNumber = 0;
      tree.cacheHasLine = false;
      tree.cacheValid = true;
      return makeNodePos(x2, rest - sizeLeft, nodeStartOffset);
    } else {
      rest = rest - sizeLeft - pieceLen | 0;
      nodeStartOffset = nodeStartOffset + sizeLeft + pieceLen | 0;
      x2 = x2.right;
    }
    guard++;
  }
  return makeNodePos(sentinel, 0, nodeStartOffset);
}
function hostNodeAt2(tree, lineNumber, column, sentinel) {
  let line = lineNumber | 0;
  let col = column | 0;
  let x2 = tree.root;
  let nodeStartOffset = 0;
  let guard = 0;
  while (x2 !== sentinel && guard < 1e5) {
    const lfLeft = x2.lf_left | 0;
    const pieceLf = x2.piece.lineFeedCnt | 0;
    const xLeft = x2.left;
    if (xLeft !== sentinel && lfLeft >= line - 1) {
      x2 = xLeft;
    } else if (lfLeft + pieceLf > line - 1) {
      const prevAccumulatedValue = hostAccumulatedValue(tree, x2, line - lfLeft - 2);
      const accumulatedValue = hostAccumulatedValue(tree, x2, line - lfLeft - 1);
      nodeStartOffset = nodeStartOffset + x2.size_left | 0;
      let rem = prevAccumulatedValue + col - 1 | 0;
      if (rem > accumulatedValue) {
        rem = accumulatedValue;
      }
      return makeNodePos(x2, rem, nodeStartOffset);
    } else if (lfLeft + pieceLf === line - 1) {
      const prevAccumulatedValue = hostAccumulatedValue(tree, x2, line - lfLeft - 2);
      const pieceLen = x2.piece.length | 0;
      if (prevAccumulatedValue + col - 1 <= pieceLen) {
        return makeNodePos(x2, prevAccumulatedValue + col - 1 | 0, nodeStartOffset);
      }
      col = col - (pieceLen - prevAccumulatedValue) | 0;
      break;
    } else {
      line = line - lfLeft - pieceLf | 0;
      nodeStartOffset = nodeStartOffset + x2.size_left + x2.piece.length | 0;
      x2 = x2.right;
    }
    guard++;
  }
  x2 = hostNodeNext(x2, sentinel);
  let walk = 0;
  while (x2 !== sentinel && walk < 1e5) {
    const pieceLf = x2.piece.lineFeedCnt | 0;
    const pieceLen = x2.piece.length | 0;
    if (pieceLf > 0) {
      const accumulatedValue = hostAccumulatedValue(tree, x2, 0);
      const start = hostOffsetOfNode(tree, x2);
      let rem = col - 1 | 0;
      if (rem > accumulatedValue) {
        rem = accumulatedValue;
      }
      return makeNodePos(x2, rem, start);
    } else if (pieceLen >= col - 1) {
      return makeNodePos(x2, col - 1 | 0, hostOffsetOfNode(tree, x2));
    } else {
      col = col - pieceLen | 0;
    }
    x2 = hostNodeNext(x2, sentinel);
    walk++;
  }
  return makeNodePos(sentinel, 0, nodeStartOffset);
}
function nodeAtStash(tree, offset, sentinel, slot) {
  if (!tree._stash) {
    tree._stash = [null, null];
  }
  tree._stash[slot | 0] = hostNodeAt(tree, offset, sentinel);
}
function nodeAt2Stash(tree, lineNumber, column, sentinel, slot) {
  if (!tree._stash) {
    tree._stash = [null, null];
  }
  tree._stash[slot | 0] = hostNodeAt2(tree, lineNumber, column, sentinel);
}
function stashNode(tree, slot) {
  return tree._stash[slot | 0].node;
}
function stashRem(tree, slot) {
  return tree._stash[slot | 0].remainder | 0;
}
function stashStart(tree, slot) {
  return tree._stash[slot | 0].nodeStartOffset | 0;
}
function hostCursorLine(cursor) {
  return (cursor.line ?? cursor[0] ?? 0) | 0;
}
function hostCursorCol(cursor) {
  return (cursor.column ?? cursor[1] ?? 0) | 0;
}
function hostOffsetInBuffer(tree, bufferIndex, cursor) {
  const lineStarts = tree.buffers[bufferIndex].lineStarts;
  return lineStarts[hostCursorLine(cursor)] + hostCursorCol(cursor) | 0;
}
function hostGetLineFeedCnt(tree, bufferIndex, start, end) {
  const endLine = hostCursorLine(end);
  const endCol = hostCursorCol(end);
  const startLine = hostCursorLine(start);
  if (endCol === 0) {
    return endLine - startLine | 0;
  }
  const lineStarts = tree.buffers[bufferIndex].lineStarts;
  if (endLine === lineStarts.length - 1) {
    return endLine - startLine | 0;
  }
  const nextLineStartOffset = lineStarts[endLine + 1] | 0;
  const endOffset = lineStarts[endLine] + endCol | 0;
  if (nextLineStartOffset > endOffset + 1) {
    return endLine - startLine | 0;
  }
  const previousCharOffset = endOffset - 1 | 0;
  if (tree.buffers[bufferIndex].buffer.charCodeAt(previousCharOffset) === 13) {
    return endLine - startLine + 1 | 0;
  }
  return endLine - startLine | 0;
}
function hostPositionInBuffer(tree, node, remainder) {
  const piece = node.piece;
  const lineStarts = tree.buffers[piece.bufferIndex].lineStarts;
  const startOffset = lineStarts[piece.start.line] + piece.start.column + (remainder | 0) | 0;
  let low = piece.start.line | 0;
  let high = piece.end.line | 0;
  let mid = low;
  let midStart = 0;
  let midStop = 0;
  while (low <= high) {
    mid = low + ((high - low) / 2 | 0) | 0;
    midStart = lineStarts[mid] | 0;
    if (mid === lineStarts.length - 1) {
      midStop = tree.buffers[piece.bufferIndex].buffer.length | 0;
    } else {
      midStop = lineStarts[mid + 1] | 0;
    }
    if (startOffset < midStart) {
      high = mid - 1 | 0;
    } else if (startOffset >= midStop) {
      low = mid + 1 | 0;
    } else {
      break;
    }
  }
  return { line: mid, column: startOffset - midStart | 0 };
}
function hostGetIndexOf(tree, node, accumulatedValue) {
  const piece = node.piece;
  const pos = hostPositionInBuffer(tree, node, accumulatedValue);
  const lineCnt = pos.line - piece.start.line | 0;
  const span = hostOffsetInBuffer(tree, piece.bufferIndex, piece.end) - hostOffsetInBuffer(tree, piece.bufferIndex, piece.start) | 0;
  if (span === (accumulatedValue | 0)) {
    const realLineCnt = hostGetLineFeedCnt(tree, node.piece.bufferIndex, piece.start, pos);
    if (realLineCnt !== lineCnt) {
      return { index: realLineCnt, remainder: 0 };
    }
  }
  return { index: lineCnt, remainder: pos.column | 0 };
}
function hostGetOffsetAt(tree, lineNumber, column, sentinel) {
  let leftLen = 0;
  let x2 = tree.root;
  let line = lineNumber | 0;
  let guard = 0;
  while (x2 !== sentinel && guard < 1e5) {
    guard++;
    if (x2.left !== sentinel && x2.lf_left + 1 >= line) {
      x2 = x2.left;
    } else if (x2.lf_left + x2.piece.lineFeedCnt + 1 >= line) {
      leftLen = leftLen + x2.size_left | 0;
      const acc = hostAccumulatedValue(tree, x2, line - x2.lf_left - 2 | 0);
      return leftLen + acc + (column | 0) - 1 | 0;
    } else {
      line = line - x2.lf_left - x2.piece.lineFeedCnt | 0;
      leftLen = leftLen + x2.size_left + x2.piece.length | 0;
      x2 = x2.right;
    }
  }
  return leftLen;
}
function makePos(line, column) {
  const lineN = line | 0;
  const colN = column | 0;
  const pos = [lineN, colN];
  pos.lineNumber = lineN;
  pos.column = colN;
  return pos;
}
function hostGetPositionAt(tree, offset, sentinel) {
  let rest = offset | 0;
  if (rest < 0) {
    rest = 0;
  }
  let x2 = tree.root;
  let lfCnt = 0;
  const originalOffset = rest;
  let guard = 0;
  while (x2 !== sentinel && guard < 1e5) {
    guard++;
    if (x2.size_left !== 0 && x2.size_left >= rest) {
      x2 = x2.left;
    } else if (x2.size_left + x2.piece.length >= rest) {
      const out = hostGetIndexOf(tree, x2, rest - x2.size_left | 0);
      lfCnt = lfCnt + x2.lf_left + out.index | 0;
      if (out.index === 0) {
        const lineStartOffset = hostGetOffsetAt(tree, lfCnt + 1, 1, sentinel);
        const column = originalOffset - lineStartOffset | 0;
        return makePos(lfCnt + 1, column + 1);
      }
      return makePos(lfCnt + 1, out.remainder + 1);
    } else {
      rest = rest - x2.size_left - x2.piece.length | 0;
      lfCnt = lfCnt + x2.lf_left + x2.piece.lineFeedCnt | 0;
      if (x2.right === sentinel) {
        const lineStartOffset = hostGetOffsetAt(tree, lfCnt + 1, 1, sentinel);
        const column = originalOffset - rest - lineStartOffset | 0;
        return makePos(lfCnt + 1, column + 1);
      }
      x2 = x2.right;
    }
  }
  return makePos(1, 1);
}
function hostCalcSize(node, sentinel) {
  if (node === sentinel) {
    return 0;
  }
  return node.size_left + node.piece.length + hostCalcSize(node.right, sentinel) | 0;
}
function hostCalcLf(node, sentinel) {
  if (node === sentinel) {
    return 0;
  }
  return node.lf_left + node.piece.lineFeedCnt + hostCalcLf(node.right, sentinel) | 0;
}
function hostLeftest(node, sentinel) {
  while (node.left !== sentinel) {
    node = node.left;
  }
  return node;
}
function hostLeftRotate(tree, x2, sentinel) {
  const y2 = x2.right;
  y2.size_left = y2.size_left + x2.size_left + x2.piece.length | 0;
  y2.lf_left = y2.lf_left + x2.lf_left + x2.piece.lineFeedCnt | 0;
  x2.right = y2.left;
  if (y2.left !== sentinel) {
    y2.left.parent = x2;
  }
  y2.parent = x2.parent;
  if (x2.parent === sentinel) {
    tree.root = y2;
  } else if (x2.parent.left === x2) {
    x2.parent.left = y2;
  } else {
    x2.parent.right = y2;
  }
  y2.left = x2;
  x2.parent = y2;
}
function hostRightRotate(tree, y2, sentinel) {
  const x2 = y2.left;
  y2.left = x2.right;
  if (x2.right !== sentinel) {
    x2.right.parent = y2;
  }
  x2.parent = y2.parent;
  y2.size_left = y2.size_left - (x2.size_left + x2.piece.length) | 0;
  y2.lf_left = y2.lf_left - (x2.lf_left + x2.piece.lineFeedCnt) | 0;
  if (y2.parent === sentinel) {
    tree.root = x2;
  } else if (y2 === y2.parent.right) {
    y2.parent.right = x2;
  } else {
    y2.parent.left = x2;
  }
  x2.right = y2;
  y2.parent = x2;
}
function hostUpdateMeta(tree, x2, delta, lfDelta, sentinel) {
  while (x2 !== tree.root && x2 !== sentinel) {
    if (x2.parent.left === x2) {
      x2.parent.size_left = x2.parent.size_left + delta | 0;
      x2.parent.lf_left = x2.parent.lf_left + lfDelta | 0;
    }
    x2 = x2.parent;
  }
}
function hostRecomputeMeta(tree, x2, sentinel) {
  if (x2 === tree.root) {
    return;
  }
  while (x2 !== tree.root && x2 === x2.parent.right) {
    x2 = x2.parent;
  }
  if (x2 === tree.root) {
    return;
  }
  x2 = x2.parent;
  const delta = hostCalcSize(x2.left, sentinel) - x2.size_left | 0;
  const lfDelta = hostCalcLf(x2.left, sentinel) - x2.lf_left | 0;
  x2.size_left = x2.size_left + delta | 0;
  x2.lf_left = x2.lf_left + lfDelta | 0;
  while (x2 !== tree.root && (delta !== 0 || lfDelta !== 0)) {
    if (x2.parent.left === x2) {
      x2.parent.size_left = x2.parent.size_left + delta | 0;
      x2.parent.lf_left = x2.parent.lf_left + lfDelta | 0;
    }
    x2 = x2.parent;
  }
}
function hostDetach(node) {
  node.alive = false;
  node.parent = node;
  node.left = node;
  node.right = node;
}
function rbDeleteTree(tree, z2, sentinel) {
  let y2;
  let x2;
  if (z2.left === sentinel) {
    y2 = z2;
    x2 = y2.right;
  } else if (z2.right === sentinel) {
    y2 = z2;
    x2 = y2.left;
  } else {
    y2 = hostLeftest(z2.right, sentinel);
    x2 = y2.right;
  }
  if (y2 === tree.root) {
    tree.root = x2;
    x2.color = 0;
    hostDetach(z2);
    sentinel.parent = sentinel;
    tree.root.parent = sentinel;
    return;
  }
  const yWasRed = y2.color === 1;
  if (y2 === y2.parent.left) {
    y2.parent.left = x2;
  } else {
    y2.parent.right = x2;
  }
  if (y2 === z2) {
    x2.parent = y2.parent;
    hostRecomputeMeta(tree, x2, sentinel);
  } else {
    if (y2.parent === z2) {
      x2.parent = y2;
    } else {
      x2.parent = y2.parent;
    }
    hostRecomputeMeta(tree, x2, sentinel);
    y2.left = z2.left;
    y2.right = z2.right;
    y2.parent = z2.parent;
    y2.color = z2.color;
    if (z2 === tree.root) {
      tree.root = y2;
    } else if (z2 === z2.parent.left) {
      z2.parent.left = y2;
    } else {
      z2.parent.right = y2;
    }
    if (y2.left !== sentinel) {
      y2.left.parent = y2;
    }
    if (y2.right !== sentinel) {
      y2.right.parent = y2;
    }
    y2.size_left = z2.size_left;
    y2.lf_left = z2.lf_left;
    hostRecomputeMeta(tree, y2, sentinel);
  }
  hostDetach(z2);
  if (x2.parent.left === x2) {
    const newSizeLeft = hostCalcSize(x2, sentinel);
    const newLFLeft = hostCalcLf(x2, sentinel);
    if (newSizeLeft !== x2.parent.size_left || newLFLeft !== x2.parent.lf_left) {
      const delta = newSizeLeft - x2.parent.size_left | 0;
      const lfDelta = newLFLeft - x2.parent.lf_left | 0;
      x2.parent.size_left = newSizeLeft;
      x2.parent.lf_left = newLFLeft;
      hostUpdateMeta(tree, x2.parent, delta, lfDelta, sentinel);
    }
  }
  hostRecomputeMeta(tree, x2.parent, sentinel);
  if (yWasRed) {
    sentinel.parent = sentinel;
    return;
  }
  while (x2 !== tree.root && x2.color === 0) {
    if (x2 === x2.parent.left) {
      let w2 = x2.parent.right;
      if (w2.color === 1) {
        w2.color = 0;
        x2.parent.color = 1;
        hostLeftRotate(tree, x2.parent, sentinel);
        w2 = x2.parent.right;
      }
      if (w2.left.color === 0 && w2.right.color === 0) {
        w2.color = 1;
        x2 = x2.parent;
      } else {
        if (w2.right.color === 0) {
          w2.left.color = 0;
          w2.color = 1;
          hostRightRotate(tree, w2, sentinel);
          w2 = x2.parent.right;
        }
        w2.color = x2.parent.color;
        x2.parent.color = 0;
        w2.right.color = 0;
        hostLeftRotate(tree, x2.parent, sentinel);
        x2 = tree.root;
      }
    } else {
      let w2 = x2.parent.left;
      if (w2.color === 1) {
        w2.color = 0;
        x2.parent.color = 1;
        hostRightRotate(tree, x2.parent, sentinel);
        w2 = x2.parent.left;
      }
      if (w2.left.color === 0 && w2.right.color === 0) {
        w2.color = 1;
        x2 = x2.parent;
      } else {
        if (w2.left.color === 0) {
          w2.right.color = 0;
          w2.color = 1;
          hostLeftRotate(tree, w2, sentinel);
          w2 = x2.parent.left;
        }
        w2.color = x2.parent.color;
        x2.parent.color = 0;
        w2.left.color = 0;
        hostRightRotate(tree, x2.parent, sentinel);
        x2 = tree.root;
      }
    }
  }
  x2.color = 0;
  sentinel.parent = sentinel;
}
function splitLinesHost(text) {
  const lines = [];
  let cur = "";
  for (let i2 = 0; i2 < text.length; i2++) {
    const ch2 = text.charAt(i2);
    if (ch2 === "\n") {
      lines.push(cur);
      cur = "";
    } else if (ch2 !== "\r") {
      cur += ch2;
    }
  }
  lines.push(cur);
  return lines;
}
function makeDiffChange(os, oe2, ms, me2) {
  const change = [os, oe2, ms, me2];
  change.originalStart = os;
  change.originalEnd = oe2;
  change.modifiedStart = ms;
  change.modifiedEnd = me2;
  return change;
}
var HostFastInts = class {
  pos = [0, 0, 0, 0, 0, 0, 0, 0];
  neg = [0, 0, 0, 0, 0, 0, 0, 0];
  get(idx) {
    if (idx < 0) {
      const i2 = -idx - 1;
      return i2 >= this.neg.length ? 0 : this.neg[i2];
    }
    return idx >= this.pos.length ? 0 : this.pos[idx];
  }
  set(idx, value) {
    if (idx < 0) {
      const i2 = -idx - 1;
      while (this.neg.length <= i2) {
        this.neg.push(0);
      }
      this.neg[i2] = value;
    } else {
      while (this.pos.length <= idx) {
        this.pos.push(0);
      }
      this.pos[idx] = value;
    }
  }
};
var HostFastPaths = class {
  pos = [];
  neg = [];
  get(idx) {
    if (idx < 0) {
      const i2 = -idx - 1;
      return i2 >= this.neg.length ? null : this.neg[i2];
    }
    return idx >= this.pos.length ? null : this.pos[idx];
  }
  set(idx, value) {
    if (idx < 0) {
      const i2 = -idx - 1;
      while (this.neg.length <= i2) {
        this.neg.push(null);
      }
      this.neg[i2] = value;
    } else {
      while (this.pos.length <= idx) {
        this.pos.push(null);
      }
      this.pos[idx] = value;
    }
  }
};
function hostSnake(seqX, seqY, x2, y2) {
  let xi2 = x2 | 0;
  let yi2 = y2 | 0;
  while (xi2 < seqX.length && yi2 < seqY.length && seqX[xi2] === seqY[yi2]) {
    xi2++;
    yi2++;
  }
  return xi2;
}
function hostComputeLineDiff(original, modified) {
  const seqX = splitLinesHost(original);
  const seqY = splitLinesHost(modified);
  const result = [];
  if (seqX.length === 0 && seqY.length === 0) {
    return result;
  }
  if (seqX.length === 0) {
    result.push(makeDiffChange(0, 0, 0, seqY.length));
    return result;
  }
  if (seqY.length === 0) {
    result.push(makeDiffChange(0, seqX.length, 0, 0));
    return result;
  }
  const V2 = new HostFastInts();
  const paths = new HostFastPaths();
  const first = hostSnake(seqX, seqY, 0, 0);
  V2.set(0, first);
  paths.set(0, first === 0 ? null : { prev: null, x: 0, y: 0, length: first });
  let foundK = 0;
  let done = false;
  let d2 = 0;
  const limit = seqX.length + seqY.length + 2;
  while (!done && d2 <= limit) {
    d2++;
    const lowerBound = -Math.min(d2, seqY.length + d2 % 2);
    const upperBound = Math.min(d2, seqX.length + d2 % 2);
    for (let k2 = lowerBound; k2 <= upperBound; k2 += 2) {
      let maxXofDLineTop = -1;
      if (k2 !== upperBound) {
        maxXofDLineTop = V2.get(k2 + 1);
      }
      let maxXofDLineLeft = -1;
      if (k2 !== lowerBound) {
        maxXofDLineLeft = V2.get(k2 - 1) + 1;
      }
      const x2 = Math.min(Math.max(maxXofDLineTop, maxXofDLineLeft), seqX.length);
      const y2 = x2 - k2;
      if (x2 <= seqX.length && y2 <= seqY.length) {
        const newMaxX = hostSnake(seqX, seqY, x2, y2);
        V2.set(k2, newMaxX);
        const lastPath = x2 === maxXofDLineTop ? paths.get(k2 + 1) : paths.get(k2 - 1);
        paths.set(k2, newMaxX !== x2 ? { prev: lastPath, x: x2, y: y2, length: newMaxX - x2 } : lastPath);
        if (V2.get(k2) === seqX.length && V2.get(k2) - k2 === seqY.length) {
          foundK = k2;
          done = true;
          break;
        }
      }
    }
  }
  let path = paths.get(foundK);
  let lastAligningPosS1 = seqX.length;
  let lastAligningPosS2 = seqY.length;
  while (true) {
    let endX = 0;
    let endY = 0;
    let nextX = 0;
    let nextY = 0;
    let next = null;
    let hasPath = false;
    if (path != null) {
      hasPath = true;
      endX = path.x + path.length;
      endY = path.y + path.length;
      nextX = path.x;
      nextY = path.y;
      next = path.prev;
    }
    if (endX !== lastAligningPosS1 || endY !== lastAligningPosS2) {
      result.push(makeDiffChange(endX, lastAligningPosS1, endY, lastAligningPosS2));
    }
    if (!hasPath) {
      break;
    }
    lastAligningPosS1 = nextX;
    lastAligningPosS2 = nextY;
    path = next;
  }
  const reversed = [];
  for (let r2 = result.length - 1; r2 >= 0; r2--) {
    reversed.push(result[r2]);
  }
  return reversed;
}

// build/monaco-layers/entry.raw.js
var lf = "none";
var mf = "block";
var nf = "continue";
var of = "false";
var pf = "root";
var qf = "return";
var rf = "string";
var sf = "class";
var tf = "background";
var uf = "finally";
var vf = "import";
var wf = "function";
var xf = "\n";
var yf = "default";
var zf = "";
var Af = "implements";
var Bf = "break";
var Cf = "while";
var Df = "abstract";
var Ef = "case";
var Ff = "else";
var Gf = "true";
var Hf = "number";
var If = "comment";
var Jf = "display";
var Kf = "extends";
var Mf = "type";
var Nf = "identifier";
var Of = "catch";
var Qf = "editor.action.addSelectionToNextFindMatch";
var Rf = "for";
var Sf = "enum";
var Tf = "package";
var Uf = "interface";
var Vf = "//";
var Wf = "begin";
var Xf = "const";
var Yf = "absolute";
var Zf = "internal";
var _f = "position";
var $f = "constructor";
var bg = "white-space";
var cg = "CURRENT_TIMESTAMP";
var dg = "delimiter.bracket";
var eg = "assert";
var fg = "export";
var gg = "public";
var hg = "switch";
var ig = "*/";
var jg = "/*";
var kg = "CONSTRAINT";
var lg = '[^\\\\"]+';
var mg = "and";
var ng = "private";
var og = "as";
var pg = "if";
var qg = "in";
var rg = "auto";
var sg = "from";
var tg = "null";
var ug = "then";
var vg = "with";
var wg = "throw";
var xg = "until";
var yg = "using";
var zg = "yield";
var Bg = "namespace";
var Cg = "plaintext";
var Dg = "protected";
var Eg = "pointer-events";
var Fg = "editor.action.goToDefinition";
var Gg = "editor.action.goToReferences";
var Hg = "editor.action.triggerSuggest";
var Ig = "let";
var Jg = "try";
var Kg = "</div>";
var Lg = "CREATE";
var Mg = "extern";
var Ng = "height";
var Og = "typeof";
var Pg = "AUTHORIZATION";
var Qg = "string.escape";
var Rg = "do";
var Sg = "DISTINCT";
var Tg = "external";
var Ug = "language";
var Vg = "CURRENT_DATE";
var Wg = "CURRENT_TIME";
var Xg = "CURRENT_USER";
var Yg = "editor.action.commentLine";
var Zg = "editor.action.marker.next";
var _g = "CASE";
var $g = "file";
var ah = "left";
var bh = "px";
var ch = "end";
var dh = "new";
var eh = "not";
var fh = "set";
var gh = "var";
var hh = "Enter";
var ih = "async";
var jh = "await";
var kh = "color";
var lh = "match";
var mh = "super";
var nh = "value";
var oh = "ANALYZE";
var ph = "COLLATE";
var qh = "foreach";
var rh = "keyword";
var sh = "library";
var th = "mutable";
var uh = "editor.action.gotoLine";
var vh = "instanceof";
var wh = "COLUMN";
var xh = "delete";
var yh = "double";
var zh = "inline";
var Ah = "module";
var Bh = "object";
var Ch = "option";
var Dh = "static";
var Eh = "struct";
var Fh = "unless";
var Gh = "editor.action.rename";
var Hh = " ";
var Ih = "#";
var Jh = "AND";
var Kh = "top";
var Lh = "ELSE";
var Mh = '\\"';
var Nh = "text";
var Oh = "when";
var Ph = "wrap";
var Qh = "font-size";
var Rh = "undefined";
var Sh = "expandLineSelection";
var Th = '" style="height:';
var Uh = "documentFormatting";
var Vh = "is";
var Wh = "CHECK";
var Xh = "FALSE";
var Yh = "alias";
var Zh = "final";
var _h = "input";
var $h = "label";
var ai = "macro";
var bi = "where";
var ci = "width";
var di = "delegate";
var ei = "override";
var fi = "1px solid #454545";
var gi = "cursorWordSelect";
var hi = "ALL";
var ii = "ASC";
var ji = "int";
var ki = "off";
var li = "BETWEEN";
var mi = "DEFAULT";
var ni = "dynamic";
var oi = "include";
var pi = "message";
var qi = "minimap";
var ri = "padding";
var si = "stringS";
var ti = "[a-zA-Z_][\\w]*";
var ui = "AS";
var vi = "of";
var wi = "BOTH";
var xi = "DESC";
var yi = "base";
var zi = "byte";
var Ai = "copy";
var Bi = "data";
var Ci = "elif";
var Di = "flex";
var Ei = "goto";
var Fi = "this";
var Gi = "monaco-editor ";
var Hi = "BINARY";
var Ii = "EXCEPT";
var Ji = "Escape";
var Li = "append";
var Mi = "assign";
var Ni = "before";
var Oi = "border";
var Pi = "except";
var Qi = "global";
var Ri = "record";
var Si = "repeat";
var Ti = "select";
var Ui = "source";
var Vi = "signatureHelp";
var Wi = "[{}()\\[\\]]";
var Yi = "'";
var Zi = "--";
var _i = "	";
var $i = "or";
var aj = "to";
var bj = "ANY";
var cj = "END";
var dj = '">';
var ej = "div";
var fj = "fun";
var gj = "get";
var hj = "map";
var ij = "mod";
var jj = "nil";
var kj = "ARRAY";
var lj = "BEGIN";
var mj = "CROSS";
var nj = "False";
var oj = "[^']+";
var pj = "[^*]+";
var qj = "\\\\.";
var rj = "array";
var sj = "elsif";
var tj = "event";
var uj = "float";
var vj = "infix";
var wj = "local";
var xj = "mixin";
var yj = "range";
var zj = "union";
var Aj = "fallthrough";
var Cj = "DEFERRABLE";
var Dj = "codeAction";
var Ej = "completion";
var Fj = "constraint";
var Gj = "definition";
var Hj = "deleteLeft";
var Ij = "insertText";
var Jj = "javascript";
var Lj = "*";
var Mj = ".";
var Nj = "{";
var Oj = "/\\*";
var Pj = "CAST";
var Qj = "FROM";
var Rj = "None";
var Sj = "\\*/";
var Tj = "\\d+";
var Uj = "bool";
var Vj = "char";
var Wj = "exit";
var Xj = "fail";
var Yj = "list";
var Zj = "long";
var _j = "loop";
var $j = "move";
var ak = "uint";
var bk = "void";
var ck = "ArrowDown";
var dk = "CHARACTER";
var ek = "COLLATION";
var fk = "[^\\\\']+";
var gk = "ascending";
var hk = "attribute";
var ik = "exception";
var jk = "operation";
var kk = "otherwise";
var lk = "BY";
var mk = "IN";
var nk = "\\";
var ok = "by";
var pk = "on";
var qk = "CONTAINS";
var rk = "CONTINUE";
var sk = "Function";
var tk = '[^\\"]+';
var uk = "__FILE__";
var vk = "__LINE__";
var wk = "datatype";
var xk = "debugger";
var yk = "endembed";
var zk = "endmacro";
var Ak = "hc-black";
var Bk = "inmemory";
var Ck = "markdown";
var Dk = "operator";
var Ek = "optional";
var Fk = "pre-wrap";
var Gk = "property";
var Hk = "relative";
var Jk = "textarea";
var Kk = 1e3;
var Lk = 65535;
var Z = "vs";
function q(a, b, c) {
  b < 0 && (b = 0), c > a.length && (c = a.length);
  return b >= c ? emptyBuf() : a.slice(b, c);
}
function Ob(a) {
  var b = emptyBuf(), c = 0;
  while (c < a) b += Hh, c = c + 1 | 0;
  return b;
}
function z(a, b, c, d2, e) {
  b > d2 || b == d2 && c > e ? (a.startLineNumber = d2, a.startColumn = e, a.endLineNumber = b, a.endColumn = c) : (a.startLineNumber = b, a.startColumn = c, a.endLineNumber = d2, a.endColumn = e);
}
function ya(a, b, c) {
  a.piece = b, a.color = c, a.size_left = 0, a.lf_left = 0, a.alive = true, a.parent = a, a.left = a, a.right = a;
}
function H(a) {
  for (var e, b = [0], d2 = a.length, c = 0; c < d2; c = c + 1 | 0) e = a.charCodeAt(c), 13 == e ? c + 1 < d2 && 10 == a.charCodeAt(c + 1) ? (b.push(c + 2 | 0), c++) : b.push(c + 1) : 10 == e && b.push(c + 1);
  return b;
}
function ab(a) {
  while (a.left != l) a = a.left;
  return a;
}
function Pb(a) {
  while (a.right != l) a = a.right;
  return a;
}
function ea(a) {
  if (a.right != l) return ab(a.right);
  for (var c, b = 0; ; ) {
    c = a.parent != l && b < 1e5;
    if (!c) {
      break;
    }
    b = b + 1 | 0;
    if (a.parent.left == a) break;
    a = a.parent;
  }
  return a.parent == l || b >= 1e5 ? l : a.parent;
}
function fa(a, b, c, d2) {
  a.root = l, a.buffers = [], a.lineCnt = 1, a.length = 0, a.eol = c, a.eolLength = 2, a.eolNormalized = d2, a.lastChangeBufferPos = { line: 0, column: 0 }, a.cacheNode = l, a.cacheNodeStartOffset = 0, a.cacheNodeStartLineNumber = 0, a.cacheHasLine = false, a.cacheValid = false, a.lastVisitedLineNumber = 0, a.lastVisitedLineValue = emptyBuf(), a.posNode = l, a.posRemainder = 0, a.posStart = 0, a.walkLine = 1, a.walkCol = 1, c = { buffer: "", lineStarts: [] }, c.buffer = emptyBuf(), c.lineStarts = [0], a.tmpBuffer = c, Qb(a, b);
}
function Qb(a, b) {
  var c = { buffer: "", lineStarts: [] };
  c.buffer = emptyBuf(), c.lineStarts = [0], a.buffers = [c], a.lastChangeBufferPos = { line: 0, column: 0 }, a.root = l, a.lineCnt = 1, a.length = 0, a.eol = xf, a.eolLength = 2, a.eolNormalized = true;
  var d2 = l;
  for (c = 0; c < b.length; c++) d2 = Rb(a, d2, b[c], c + 1);
  a.cacheValid = false, a.lastVisitedLineNumber = 0, a.lastVisitedLineValue = emptyBuf(), N(a);
}
function Rb(a, b, c, d2) {
  if (0 == c.buffer.length) return b;
  var e = c.lineStarts;
  0 == e.length && (e = H(c.buffer), c.lineStarts = e), d2 = { bufferIndex: d2, start: { line: 0, column: 0 }, end: { line: e.length - 1, column: c.buffer.length - (e[e.length - 1] | 0) | 0 }, lineFeedCnt: e.length - 1, length: c.buffer.length }, a.buffers.push(c);
  return b == l ? M(a, l, d2) : M(a, b, d2);
}
function bb(a, b) {
  a != l && (a.parent = b);
}
function cb(a, b) {
  var c = b.right, e = c.left, d2 = b.parent;
  c.size_left = c.size_left + (b.size_left + b.piece.length | 0) | 0, c.lf_left = c.lf_left + (b.lf_left + b.piece.lineFeedCnt | 0) | 0, b.right = e, bb(e, b), c.parent = d2, d2 == l ? a.root = c : d2.left == b ? d2.left = c : d2.right = c, c.left = b, b.parent = c;
}
function db(a, b) {
  var c = b.left, e = c.right, d2 = b.parent;
  b.left = e, bb(e, b), c.parent = d2, b.size_left = b.size_left - (c.size_left + c.piece.length | 0) | 0, b.lf_left = b.lf_left - (c.lf_left + c.piece.lineFeedCnt | 0) | 0, d2 == l ? a.root = c : b == d2.right ? d2.right = c : d2.left = c, c.right = b, b.parent = c;
}
function eb(a, b, c) {
  var d2 = a.parent;
  d2.left == a && (d2.size_left = d2.size_left + b | 0, d2.lf_left = d2.lf_left + c | 0);
}
function ga(a, b, c, d2) {
  while (b != a.root && b != l) eb(b, c, d2), b = b.parent;
}
var Sb = /* @__PURE__ */ (function() {
  function a(b2) {
    return b2 == l ? 0 : (b2.size_left + b2.piece.length | 0) + a(b2.right) | 0;
  }
  function b(a2) {
    return a2 == l ? 0 : (a2.lf_left + a2.piece.lineFeedCnt | 0) + b(a2.right) | 0;
  }
  return function(c, d2) {
    if (d2 == c.root) return;
    for (; ; ) {
      var e = d2 != c.root && d2 == d2.parent.right;
      if (!e) {
        break;
      }
      d2 = d2.parent;
    }
    if (d2 == c.root) return;
    d2 = d2.parent, e = a(d2.left) - d2.size_left | 0;
    var f = b(d2.left) - d2.lf_left | 0;
    d2.size_left = d2.size_left + e | 0, d2.lf_left = d2.lf_left + f | 0;
    while (d2 != c.root && (0 != e || 0 != f)) eb(d2, e, f), d2 = d2.parent;
  };
})();
function fb(a, b) {
  Sb(a, b);
  for (; ; ) {
    var c = b != a.root && 1 == b.parent.color;
    if (!c) {
      break;
    }
    b.parent == b.parent.parent.left ? (c = b.parent.parent.right, 1 == c.color ? (b.parent.color = 0, c.color = 0, b.parent.parent.color = 1, b = b.parent.parent) : (b == b.parent.right && (b = b.parent, cb(a, b)), b.parent.color = 0, b.parent.parent.color = 1, db(a, b.parent.parent))) : (c = b.parent.parent.left, 1 == c.color ? (b.parent.color = 0, c.color = 0, b.parent.parent.color = 1, b = b.parent.parent) : (b == b.parent.left && (b = b.parent, db(a, b)), b.parent.color = 0, b.parent.parent.color = 1, cb(a, b.parent.parent)));
  }
  a.root.color = 0;
}
function M(a, b, c) {
  var d2 = { parent: null, left: null, right: null, color: 0, piece: null, size_left: 0, lf_left: 0, alive: false };
  ya(d2, c, 1), d2.left = l, d2.right = l, d2.parent = l, d2.size_left = 0, d2.lf_left = 0, a.root == l ? (a.root = d2, d2.color = 0) : b.right == l ? (b.right = d2, d2.parent = b) : (b = ab(b.right), b.left = d2, d2.parent = b), fb(a, d2);
  return d2;
}
function za(a, b, c) {
  var d2 = { parent: null, left: null, right: null, color: 0, piece: null, size_left: 0, lf_left: 0, alive: false };
  ya(d2, c, 1), d2.left = l, d2.right = l, d2.parent = l, d2.size_left = 0, d2.lf_left = 0, a.root == l ? (a.root = d2, d2.color = 0) : b.left == l ? (b.left = d2, d2.parent = b) : (b = Pb(b.left), b.right = d2, d2.parent = b), fb(a, d2);
  return d2;
}
function N(a) {
  var b = a.root, c = 1, d2 = 0;
  while (b != l) c = c + (b.lf_left + b.piece.lineFeedCnt | 0) | 0, d2 = d2 + (b.size_left + b.piece.length | 0) | 0, b = b.right;
  a.lineCnt = c, a.length = d2, ha(a, a.length);
}
function ha(a, b) {
  a.cacheValid && (!a.cacheNode.alive || a.cacheNodeStartOffset >= b) && (a.cacheValid = false);
}
function A(a, b, c) {
  return (a.buffers[b].lineStarts[c.line] | 0) + c.column | 0;
}
function R(a, b, c, d2) {
  if (0 == d2.column) return d2.line - c.line | 0;
  var e = a.buffers[b].lineStarts;
  if (d2.line == e.length - 1) return d2.line - c.line | 0;
  var f = (e[d2.line] | 0) + d2.column | 0;
  return (e[d2.line + 1 | 0] | 0) > (f + 1 | 0) ? d2.line - c.line | 0 : 13 == a.buffers[b].buffer.charCodeAt(f - 1 | 0) ? (d2.line - c.line | 0) + 1 | 0 : d2.line - c.line | 0;
}
function _(a, b, c) {
  var d2 = b.piece, g = a.buffers[d2.bufferIndex].lineStarts, h = ((g[d2.start.line] | 0) + d2.start.column | 0) + c | 0;
  c = d2.start.line;
  var i2, e = d2.end.line, b = c, f = 0;
  while (c <= e) {
    b = c + ((e - c | 0) / 2 | 0) | 0, f = g[b] | 0, i2 = b == g.length - 1 ? a.buffers[d2.bufferIndex].buffer.length : g[b + 1 | 0] | 0;
    if (h < f) {
      e = b - 1 | 0;
    } else {
      if (h >= i2) {
        c = b + 1 | 0;
      } else {
        break;
      }
    }
  }
  return { line: b, column: h - f | 0 };
}
function Tb(a, b) {
  if (b == l) return emptyBuf();
  b = b.piece;
  return q(a.buffers[b.bufferIndex].buffer, A(a, b.bufferIndex, b.start), A(a, b.bufferIndex, b.end));
}
function gb(a) {
  var b = stashNode(a, 0), e = stashNode(a, 1), c = stashRem(a, 0), f = stashRem(a, 1);
  if (b == e) {
    var d2 = A(a, b.piece.bufferIndex, b.piece.start);
    return q(a.buffers[b.piece.bufferIndex].buffer, d2 + c | 0, d2 + f | 0);
  }
  d2 = A(a, b.piece.bufferIndex, b.piece.start);
  c = q(a.buffers[b.piece.bufferIndex].buffer, d2 + c | 0, d2 + b.piece.length | 0), b = ea(b);
  var g = 0;
  while (b != l && g < 1e5) {
    d2 = A(a, b.piece.bufferIndex, b.piece.start);
    if (b == e) {
      c = concat2(c, q(a.buffers[b.piece.bufferIndex].buffer, d2, d2 + f | 0));
      break;
    }
    c = concat2(c, q(a.buffers[b.piece.bufferIndex].buffer, d2, d2 + b.piece.length | 0));
    b = ea(b), g = g + 1 | 0;
  }
  return c;
}
function Aa(a, b) {
  if (b.startLineNumber == b.endLineNumber && b.startColumn == b.endColumn) return emptyBuf();
  nodeAt2Stash(a, b.startLineNumber, b.startColumn, l, 0), nodeAt2Stash(a, b.endLineNumber, b.endColumn, l, 1);
  return gb(a);
}
function I(a, b) {
  return b == l ? emptyBuf() : concat2(concat2(I(a, b.left), Tb(a, b)), I(a, b.right));
}
function hb(a, b) {
  if (a.lastVisitedLineNumber == b) return a.lastVisitedLineValue;
  a.lastVisitedLineNumber = b;
  if (b == a.lineCnt) {
    a.lastVisitedLineValue = Ba(a, b, 0);
  } else {
    if (a.eolNormalized) {
      a.lastVisitedLineValue = Ba(a, b, a.eolLength);
    } else {
      a.lastVisitedLineValue = Ba(a, b, 0).replace(/\r\n|\r|\n/g, emptyBuf());
    }
  }
  return a.lastVisitedLineValue;
}
function Ba(a, b, c) {
  var d2 = hostGetOffsetAt(a, b, 1, l);
  b = b == a.lineCnt ? a.length : hostGetOffsetAt(a, b + 1 | 0, 1, l) - c, b < d2 && (b = d2), nodeAtStash(a, d2, l, 0), nodeAtStash(a, b, l, 1);
  return gb(a);
}
function Ub(a, b) {
  if (b == a.lineCnt) {
    var c = a.length;
    return c - hostGetOffsetAt(a, b, 1, l) | 0;
  }
  c = hostGetOffsetAt(a, b + 1 | 0, 1, l);
  return (c - hostGetOffsetAt(a, b, 1, l) | 0) - a.eolLength | 0;
}
function ia(a, b) {
  if (b.length > Lk) {
    var e = [];
    while (b.length > Lk) {
      var d2 = b.charCodeAt(65534);
      13 == d2 || d2 >= 55296 && d2 <= 56319 ? (d2 = q(b, 0, 65534), b = q(b, 65534, b.length)) : (d2 = q(b, 0, Lk), b = q(b, Lk, b.length));
      var c = H(d2);
      e.push({ bufferIndex: a.buffers.length, start: { line: 0, column: 0 }, end: { line: c.length - 1, column: d2.length - (c[c.length - 1] | 0) | 0 }, lineFeedCnt: c.length - 1, length: d2.length }), pushNewBuffer(a, d2, c);
    }
    d2 = H(b);
    e.push({ bufferIndex: a.buffers.length, start: { line: 0, column: 0 }, end: { line: d2.length - 1, column: b.length - (d2[d2.length - 1] | 0) | 0 }, lineFeedCnt: d2.length - 1, length: b.length }), pushNewBuffer(a, b, d2);
    return e;
  }
  e = a.buffers[0].buffer.length;
  d2 = H(b);
  var f = a.lastChangeBufferPos;
  if (0 != e) {
    for (c = 0; c < d2.length; c++) d2[c] = (d2[c] | 0) + e | 0;
  }
  a.buffers[0].lineStarts = a.buffers[0].lineStarts.concat(d2.slice(1));
  d2 = a.buffers[0], d2.buffer = concat2(a.buffers[0].buffer, b), d2 = a.buffers[0].buffer.length, b = a.buffers[0].lineStarts.length - 1, b = { line: b, column: d2 - (a.buffers[0].lineStarts[b] | 0) | 0 }, d2 = { bufferIndex: 0, start: f, end: b, lineFeedCnt: R(a, 0, f, b), length: d2 - e }, a.lastChangeBufferPos = b;
  return [d2];
}
function Ca(a, b, c) {
  let d2 = b.piece, f = d2.lineFeedCnt, e = R(a, d2.bufferIndex, d2.start, c), i2 = e - f | 0;
  f = A(a, d2.bufferIndex, c) - A(a, d2.bufferIndex, d2.end) | 0, b.piece = { bufferIndex: d2.bufferIndex, start: d2.start, end: c, lineFeedCnt: e, length: d2.length + f | 0 }, ga(a, b, f, i2);
}
function ib(a, b, c) {
  let d2 = b.piece, f = d2.lineFeedCnt, e = R(a, d2.bufferIndex, c, d2.end), h = e - f | 0;
  f = A(a, d2.bufferIndex, d2.start) - A(a, d2.bufferIndex, c) | 0, b.piece = { bufferIndex: d2.bufferIndex, start: c, end: d2.end, lineFeedCnt: e, length: d2.length + f | 0 }, ga(a, b, f, h);
}
function Vb(a, b, h) {
  var e = a.buffers[0].buffer.length, d2 = a.buffers[0];
  d2.buffer = concat2(a.buffers[0].buffer, h), d2 = H(h);
  for (var f, c = 0; c < d2.length; c++) d2[c] = (d2[c] | 0) + e | 0;
  a.buffers[0].lineStarts = a.buffers[0].lineStarts.concat(d2.slice(1)), d2 = a.buffers[0].lineStarts.length - 1, d2 = { line: d2, column: a.buffers[0].buffer.length - (a.buffers[0].lineStarts[d2] | 0) | 0 }, e = b.piece.length + h.length | 0, f = b.piece.lineFeedCnt, c = R(a, 0, b.piece.start, d2), b.piece = { bufferIndex: b.piece.bufferIndex, start: b.piece.start, end: d2, lineFeedCnt: c, length: e }, a.lastChangeBufferPos = d2, ga(a, b, h.length, c - f | 0);
}
function Wb(a, h, b) {
  var c = ia(a, h);
  b = za(a, b, c[c.length - 1]), h = c.length - 2;
  while (h >= 0) b = za(a, b, c[h]), h--;
}
function Xb(a, h, b) {
  h = ia(a, h), b = M(a, b, h[0]);
  for (var c = 1; c < h.length; c++) b = M(a, b, h[c]);
}
function Yb(a, b, h) {
  a.lastVisitedLineNumber = 0, a.lastVisitedLineValue = emptyBuf();
  if (a.root != l) {
    nodeAtStash(a, b, l, 0);
    var c = stashNode(a, 0), g = stashRem(a, 0), f = stashStart(a, 0), d2 = c.piece;
    if (0 == d2.bufferIndex && d2.end.line == a.lastChangeBufferPos.line && d2.end.column == a.lastChangeBufferPos.column && (f + d2.length | 0) == b && h.length < Lk) {
      Vb(a, c, h), N(a);
      return;
    }
    if (f == b) {
      Wb(a, h, c), ha(a, b);
    } else {
      if ((f + d2.length | 0) > b) {
        for (b = _(a, c, g), g = R(a, d2.bufferIndex, b, d2.end), d2 = { bufferIndex: d2.bufferIndex, start: b, end: d2.end, lineFeedCnt: g, length: A(a, d2.bufferIndex, d2.end) - A(a, d2.bufferIndex, b) | 0 }, Ca(a, c, b), h = ia(a, h), d2.length > 0 && M(a, c, d2), b = 0; b < h.length; b++) c = M(a, c, h[b]);
      } else {
        Xb(a, h, c);
      }
    }
  } else {
    for (b = ia(a, h), h = za(a, l, b[0]), c = 1; c < b.length; c++) h = M(a, h, b[c]);
  }
  N(a);
}
function Zb(a, c, b) {
  a.lastVisitedLineNumber = 0, a.lastVisitedLineValue = emptyBuf();
  var d2, e, f, g, h;
  if (b <= 0 || a.root == l) return;
  nodeAtStash(a, c, l, 0), nodeAtStash(a, c + b | 0, l, 1), d2 = stashNode(a, 0), e = stashNode(a, 1), f = stashRem(a, 0), g = stashRem(a, 1), h = stashStart(a, 0);
  if (d2 == e) {
    e = _(a, d2, f), f = _(a, d2, g);
    if (h == c) {
      if (b == d2.piece.length) {
        rbDeleteTree(a, d2, l), N(a);
        return;
      }
      ib(a, d2, f);
      ha(a, c), N(a);
      return;
    }
    if ((h + d2.piece.length | 0) == (c + b | 0)) {
      Ca(a, d2, e), N(a);
      return;
    }
    c = d2.piece;
    h = c.start, b = c.end, g = R(a, c.bufferIndex, h, e), h = A(a, c.bufferIndex, e) - A(a, c.bufferIndex, h) | 0, d2.piece = { bufferIndex: c.bufferIndex, start: c.start, end: e, lineFeedCnt: g, length: h }, ga(a, d2, h - c.length | 0, g - c.lineFeedCnt | 0), g = R(a, c.bufferIndex, f, b), M(a, d2, { bufferIndex: c.bufferIndex, start: f, end: b, lineFeedCnt: g, length: A(a, c.bufferIndex, b) - A(a, c.bufferIndex, f) | 0 }), N(a);
    return;
  }
  b = [];
  Ca(a, d2, _(a, d2, f)), ha(a, c), 0 == d2.piece.length && b.push(d2), ib(a, e, _(a, e, g)), 0 == e.piece.length && b.push(e), c = ea(d2);
  while (c != l && c != e) b.push(c), c = ea(c);
  for (c = 0; c < b.length; c++) rbDeleteTree(a, b[c], l);
  N(a);
}
function X(a) {
  var d2 = a.scheme, b = d2 + ":";
  (a.authority.length > 0 || d2 == $g || d2 == Bk) && (b = b + Vf + a.authority), b += a.path, a.query.length > 0 && (b = b + "?" + a.query), a.fragment.length > 0 && (b = b + Ih + a.fragment);
  return b;
}
function _b(a) {
  var b = [zf, a];
  if (!a.startsWith(Vf)) return b;
  a = q(a, 2, a.length);
  var c = a.indexOf("/");
  if (c < 0) {
    b[0] = a, b[1] = zf;
    return b;
  }
  b[0] = q(a, 0, c);
  b[1] = q(a, c, a.length);
  return b;
}
function kd(a) {
  var b = a.indexOf(":"), c = $g;
  b >= 0 && (c = q(a, 0, b), a = q(a, b + 1 | 0, a.length));
  var e = _b(a);
  a = e[1] || "";
  var d2 = a.indexOf(Ih);
  if (d2 >= 0) {
    var f = q(a, d2 + 1 | 0, a.length);
    a = q(a, 0, d2);
  } else {
    f = zf;
  }
  d2 = a.indexOf("?");
  d2 >= 0 && (b = q(a, d2 + 1 | 0, a.length), a = q(a, 0, d2)), a = [a, b, f], b = e[0] || "", d2 = a[0] || "", e = a[1] || "", f = a[2] || "", a = { scheme: "", authority: "", path: "", query: "", fragment: "" }, a.scheme = c, a.authority = b, a.path = d2, a.query = e, a.fragment = f;
  return a;
}
function ld(a) {
  let b = { scheme: "", authority: "", path: "", query: "", fragment: "" };
  b.scheme = $g, b.authority = zf, b.path = a, b.query = zf, b.fragment = zf;
  return b;
}
function B(c) {
  var a = c.listeners.slice();
  c = 0;
  while (c < a.length) a[c](), c++;
}
function jb(a, b) {
  a.deco = b, a.color = 0, a.maxEndLine = b.range.endLineNumber, a.maxEndColumn = b.range.endColumn, a.alive = true, a.parent = a, a.left = a, a.right = a;
}
function $b(a, b) {
  return a.endLineNumber < b.startLineNumber || a.endLineNumber == b.startLineNumber && a.endColumn < b.startColumn ? false : b.endLineNumber < a.startLineNumber || b.endLineNumber == a.startLineNumber && b.endColumn < a.startColumn ? false : true;
}
function ja(a, b) {
  a != r && (a.parent = b);
}
var ac;
var bc;
var dc;
var ec;
(function() {
  function a(a2, b2) {
    var c = b2.right, e = c.left, d2 = b2.parent;
    b2.right = e, ja(e, b2), c.parent = d2, d2 == r ? a2.root = c : d2.left == b2 ? d2.left = c : d2.right = c, c.left = b2, b2.parent = c, $(b2), $(c);
  }
  function b(a2, b2) {
    var c = b2.left, e = c.right, d2 = b2.parent;
    b2.left = e, ja(e, b2), c.parent = d2, d2 == r ? a2.root = c : d2.right == b2 ? d2.right = c : d2.left = c, c.right = b2, b2.parent = c, $(b2), $(c);
  }
  ac = function(c, d2) {
    var e = d2.parent.parent.right;
    if (1 == e.color) {
      e.color = 0, d2.parent.color = 0, d2.parent.parent.color = 1;
      return d2.parent.parent;
    }
    d2 == d2.parent.right && (d2 = d2.parent, a(c, d2));
    d2.parent.color = 0, d2.parent.parent.color = 1, b(c, d2.parent.parent);
    return d2;
  };
  bc = function(c, d2) {
    var e = d2.parent.parent.left;
    if (1 == e.color) {
      e.color = 0, d2.parent.color = 0, d2.parent.parent.color = 1;
      return d2.parent.parent;
    }
    d2 == d2.parent.left && (d2 = d2.parent, b(c, d2));
    d2.parent.color = 0, d2.parent.parent.color = 1, a(c, d2.parent.parent);
    return d2;
  }, dc = function(c, d2) {
    var e = d2.parent, f = e.right;
    1 == f.color && (f.color = 0, e.color = 1, a(c, e), e = d2.parent, f = e.right);
    var h = f.left, g = f.right;
    if (0 == h.color && 0 == g.color) {
      f.color = 1;
      return e;
    }
    0 == g.color && (h.color = 0, f.color = 1, b(c, f), e = d2.parent, f = e.right, g = e.right.right);
    f.color = e.color, e.color = 0, g.color = 0, a(c, e);
    return c.root;
  }, ec = function(c, d2) {
    var e = d2.parent, f = e.left;
    1 == f.color && (f.color = 0, e.color = 1, b(c, e), e = d2.parent, f = e.left);
    var g = f.left, h = f.right;
    if (0 == h.color && 0 == g.color) {
      f.color = 1;
      return e;
    }
    0 == g.color && (h.color = 0, f.color = 1, a(c, f), e = d2.parent, f = e.left, g = e.left.left);
    f.color = e.color, e.color = 0, g.color = 0, b(c, e);
    return c.root;
  };
})();
function kb(a) {
  while (a != r) $(a), a = a.parent;
}
function $(a) {
  var e = a.deco.range.endLineNumber, b = a.deco.range.endColumn;
  a.left != r && (a.left.maxEndLine > e || a.left.maxEndLine == e && a.left.maxEndColumn > b) && (e = a.left.maxEndLine, b = a.left.maxEndColumn), a.right != r && (a.right.maxEndLine > e || a.right.maxEndLine == e && a.right.maxEndColumn > b) && (e = a.right.maxEndLine, b = a.right.maxEndColumn), a.maxEndLine = e, a.maxEndColumn = b;
}
function Da(a, b, c, d2) {
  if (b == r) return;
  if (b.maxEndLine < c.startLineNumber || b.maxEndLine == c.startLineNumber && b.maxEndColumn < c.startColumn) return;
  Da(a, b.left, c, d2), $b(b.deco.range, c) && d2.push(b.deco), (b.deco.range.startLineNumber < c.endLineNumber || b.deco.range.startLineNumber == c.endLineNumber && b.deco.range.startColumn <= c.endColumn) && Da(a, b.right, c, d2);
}
function Ea(a, b, c) {
  if (b == r) return r;
  if (b.deco.id == c) return b;
  var d2 = Ea(a, b.left, c);
  return d2 != r ? d2 : Ea(a, b.right, c);
}
function ka(a, b, c) {
  var d2 = b.parent;
  d2 == r ? a.root = c : d2.left == b ? d2.left = c : d2.right = c, c.parent = d2;
}
function cc(a) {
  while (a.left != r) a = a.left;
  return a;
}
function fc(a, b) {
  while (b != a.root && 0 == b.color) b = b.parent.left == b ? dc(a, b) : ec(a, b);
  b.color = 0;
}
function gc(a, b) {
  a.deleteWasY = b, a.deleteWasX = r, a.deleteWasRed = 1 == b.color;
  if (b.left == r) {
    var c = b.right;
    a.deleteWasX = c, ka(a, b, c);
  } else {
    if (b.right == r) {
      c = b.left, a.deleteWasX = c, ka(a, b, c);
    } else {
      c = cc(b.right);
      var d2 = c.right;
      a.deleteWasY = c, a.deleteWasRed = 1 == c.color, a.deleteWasX = d2, hc(a, b, c);
    }
  }
  c = a.deleteWasX;
  b.alive = false, kb(c.parent), a.deleteWasRed || fc(a, c), r.parent = r, r.left = r, r.right = r;
}
function hc(a, b, c) {
  var d2 = c.right;
  c.parent == b ? d2.parent = c : (ka(a, c, d2), c.right = b.right, ja(d2, c)), ka(a, b, c), c.left = b.left, ja(c.left, c), c.color = b.color;
}
function ic(a, b) {
  b = Ea(a, a.root, b), b != r && gc(a, b);
}
function jc(a) {
  if (0 == a.past.length) return null;
  var b = a.past[a.past.length - 1];
  a.past.splice(a.past.length - 1, 1), a.versionId = a.versionId + 1 | 0;
  return b;
}
function kc(a) {
  if (0 == a.future.length) return null;
  var b = a.future[a.future.length - 1];
  a.future.splice(a.future.length - 1, 1), a.versionId = a.versionId + 1 | 0;
  return b;
}
function E(a, g, e, b, d2, f) {
  var h = [];
  if (0 == g.length || f <= 0) return h;
  var i2 = b ? "g" : "gi";
  if (!e) {
    b = emptyBuf();
    var k2, c = 0;
    while (c < g.length) e = g.charAt(c), b = e == nk || "^" == e || "$" == e || e == Mj || "|" == e || "?" == e || e == Lj || "+" == e || "(" == e || ")" == e || "[" == e || "]" == e || e == Nj || "}" == e ? b + nk + e : b + e, c++;
    g = b;
  }
  d2 && (g = "\\b(?:" + g + ")\\b");
  b = new RegExp(g, i2), e = 1;
  while (e <= a.lineCnt && h.length < f) {
    d2 = hb(a, e), g = b.exec(d2);
    while (g && h.length < f) c = +g.index | 0, g = g[0] + "", i2 = (c + g.length | 0) + 1 | 0, c = c + 1 | 0, k2 = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 }, z(k2, e, c, e, i2), c = [g], g = { range: null, matches: [] }, g.range = k2, g.matches = c, h.push(g), g = b.exec(d2);
    e = e + 1 | 0;
  }
  return h;
}
function Fa(a) {
  if (0 == a.length) return false;
  var b = a.charCodeAt(0);
  return b >= 48 && b <= 57 ? true : b >= 65 && b <= 90 ? true : b >= 97 && b <= 122 ? true : "_" == a || "$" == a;
}
function O(a) {
  var d2, c = a.length, b = 0;
  while (b < c) {
    d2 = a.charAt(b);
    if (d2 != Hh && d2 != _i) return b;
    b++;
  }
  return c;
}
function x(e, a) {
  var b = a - 1 | 0;
  b < 0 && (b = 0), b > e.length || (a = b), b = 0;
  while (a > b) {
    if (!Fa(e.charAt((a - b | 0) - 1 | 0))) break;
    b = b + 1 | 0;
  }
  var c = a - b | 0, d2 = e.length;
  b = 0;
  while ((a + b | 0) < d2) {
    if (!Fa(e.charAt(a + b | 0))) break;
    b = b + 1 | 0;
  }
  b = a + b | 0;
  a = ["", 0, 0], a[0] = q(e, c, b), a[1] = c + 1 | 0, a[2] = b + 1 | 0;
  return a;
}
function lb(a, h, b, c) {
  let e = h.replace(/\r\n|\r|\n/g, xf), f = H(e);
  h = { buffer: "", lineStarts: [] }, h.buffer = e, h.lineStarts = f, h = [h], e = { root: null, buffers: [], lineCnt: 0, length: 0, eol: "", eolLength: 0, eolNormalized: false, lastChangeBufferPos: null, cacheNode: null, cacheNodeStartOffset: 0, cacheNodeStartLineNumber: 0, cacheHasLine: false, cacheValid: false, lastVisitedLineNumber: 0, lastVisitedLineValue: "", posNode: null, posRemainder: 0, posStart: 0, walkLine: 0, walkCol: 0, tmpBuffer: null }, fa(e, h, xf, true), a.buffer = e, a.languageId = b, a.uri = c, h = { past: [], future: [], versionId: 0 }, h.past = [], h.future = [], h.versionId = 1, a.stack = h, h = { root: null, nextId: 0, deleteWasY: null, deleteWasX: null, deleteWasRed: false }, h.root = r, h.nextId = 1, h.deleteWasY = r, h.deleteWasX = r, h.deleteWasRed = false, a.decorations = h, h = { listeners: [], disposed: false }, h.listeners = [], h.disposed = false, a.onDidChangeContent = h, a.versionId = 1, a.decoScratchIdx = 0;
}
function u(a) {
  return a.buffer.lineCnt;
}
function n(a, b) {
  return hb(a.buffer, b);
}
function t(a, b) {
  return Ub(a.buffer, b);
}
function o(a, b, d2) {
  for (var g, h, i2, j2, c, k2, m2, e = [], f = 0; f < b.length; f++) e.push(b[f]);
  b = 0;
  while (b < e.length) {
    f = b + 1;
    while (f < e.length) g = e[b], h = e[f], i2 = g.range.startLineNumber, j2 = g.range.startColumn, c = h.range.startLineNumber, k2 = h.range.startColumn, (c > i2 || c == i2 && k2 > j2) && (e[b] = h, e[f] = g), f = f + 1 | 0;
    b++;
  }
  g = [];
  c = 0;
  while (c < e.length) h = e[c], b = h.range, i2 = concat2(emptyBuf(), h.text), f = hostGetOffsetAt(a.buffer, b.startLineNumber, b.startColumn, l), j2 = hostGetOffsetAt(a.buffer, b.endLineNumber, b.endColumn, l), k2 = a.buffer, k2 = q(I(k2, k2.root), f, j2), j2 > f && Zb(a.buffer, f, j2 - f | 0), i2.length > 0 && Yb(a.buffer, f, i2), f = hostGetPositionAt(a.buffer, f + i2.length | 0, l), m2 = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 }, z(m2, b.startLineNumber, b.startColumn, f.lineNumber, f.column), f = concat2(emptyBuf(), k2), b = { range: null, text: "", identifier: 0 }, b.range = m2, b.text = f, b.identifier = h.identifier, g.push(b), c++;
  a.versionId = a.versionId + 1 | 0, d2 && (b = a.stack, b.past.push(g), b.future = [], b.versionId = b.versionId + 1 | 0), B(a.onDidChangeContent);
  return g;
}
function Ga(a) {
  var b = jc(a.stack);
  if (!b) return false;
  a.stack.future.push(o(a, b, false));
  return true;
}
function Ha(a) {
  var b = kc(a.stack);
  if (!b) return false;
  a.stack.past.push(o(a, b, false));
  return true;
}
function mb(a, b) {
  var c = 0;
  while (c < b.length) ic(a.decorations, b[c] | 0), c++;
}
var nb = /* @__PURE__ */ (function() {
  function a(a2, b2) {
    return a2.startLineNumber != b2.startLineNumber ? a2.startLineNumber < b2.startLineNumber : a2.startColumn != b2.startColumn ? a2.startColumn < b2.startColumn : a2.endLineNumber != b2.endLineNumber ? a2.endLineNumber < b2.endLineNumber : a2.endColumn < b2.endColumn;
  }
  function b(b2, c, d2, e, f) {
    var h = b2.nextId;
    b2.nextId = h + 1 | 0;
    var g = { id: 0, range: null, className: "", hoverMessage: "", isWholeLine: false };
    g.id = h, g.range = c, g.className = d2, g.hoverMessage = e, g.isWholeLine = f, d2 = { parent: null, left: null, right: null, color: 0, deco: null, maxEndLine: 0, maxEndColumn: 0, alive: false }, jb(d2, g), d2.color = 1, d2.left = r, d2.right = r, d2.parent = r;
    if (b2.root == r) {
      b2.root = d2, d2.color = 0;
      return g;
    }
    e = b2.root;
    f = r;
    while (e != r) f = a(c, e.deco.range) ? e.left : e.right, h = e, e = f, f = h;
    d2.parent = f, a(c, f.deco.range) ? f.left = d2 : f.right = d2, kb(d2);
    while (1 == d2.parent.color) {
      d2 = d2.parent == d2.parent.parent.left ? ac(b2, d2) : bc(b2, d2);
      if (d2 == b2.root) break;
    }
    b2.root.color = 0;
    return g;
  }
  return function(a2, c) {
    beginIdList(a2), a2.decoScratchIdx = 0;
    while (a2.decoScratchIdx < c.length) {
      var d2 = c[a2.decoScratchIdx];
      pushId(a2, b(a2.decorations, d2.range, d2.className, d2.hoverMessage, d2.isWholeLine).id), a2.decoScratchIdx = a2.decoScratchIdx + 1 | 0;
    }
    return takeIdList(a2);
  };
})();
function lc(h, a = "plaintext") {
  let b = wa;
  wa = wa + 1 | 0;
  let f = zf, c = "/model/" + b.toString(10);
  b = { scheme: "", authority: "", path: "", query: "", fragment: "" }, b.scheme = Bk, b.authority = zf, b.path = c, b.query = zf, b.fragment = zf, f = { buffer: null, uri: null, languageId: "", stack: null, decorations: null, onDidChangeContent: null, versionId: 0, decoScratchIdx: 0 }, lb(f, h, a, b), xa.push(f);
  return f;
}
function md(h, a, b) {
  let f = { buffer: null, uri: null, languageId: "", stack: null, decorations: null, onDidChangeContent: null, versionId: 0, decoScratchIdx: 0 };
  lb(f, h, a, b), xa.push(f);
  return f;
}
function nd() {
  return xa;
}
function v(a, b, c, d2, g) {
  let e = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
  z(e, a, b, c, d2), b = concat2(emptyBuf(), g), a = { range: null, text: "", identifier: 0 }, a.range = e, a.text = b, a.identifier = 0;
  return a;
}
function od(a, b, c, d2, e) {
  let f = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
  z(f, a, b, c, d2), a = { id: 0, range: null, className: "", hoverMessage: "", isWholeLine: false }, a.id = 0, a.range = f, a.className = e, a.hoverMessage = zf, a.isWholeLine = false;
  return a;
}
function D(a, b, c, d2, e) {
  let f = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
  z(f, b, c, d2, e), a.startLineNumber = f.startLineNumber, a.startColumn = f.startColumn, a.endLineNumber = f.endLineNumber, a.endColumn = f.endColumn, a.selectionStartLineNumber = b, a.selectionStartColumn = c, a.positionLineNumber = d2, a.positionColumn = e;
}
function S(a, b, c, d2, e) {
  a.languageId = b, a.tokenPostfix = c, a.defaultToken = d2, a.keywords = e, a.stateNames = [], a.stateRules = [], a.maxStack = 100;
}
function w(a, b) {
  if (a.stateNames.indexOf(b) >= 0) return;
  a.stateNames.push(b), a.stateRules.push([]);
}
function T() {
  return "[ \\t\\r\\n]+";
}
function i(a, b, c, d2, e, f, g) {
  w(a, b);
  var u2 = a.stateNames, h = u2.indexOf(b);
  (0 == c.length || "^" != c.charAt(0)) && (c = "^(?:" + c + ")"), b = a.stateRules[h], a = { pattern: new RegExp(), lineStart: false, kind: 0, token: "", next: "" }, a.pattern = new RegExp(c, zf), a.lineStart = g, a.kind = d2, a.token = e, a.next = f, b.push(a);
}
var Ia = /* @__PURE__ */ (function() {
  function a(a2) {
    var c2 = a2.length - 1;
    while (c2 >= 0) {
      if (a2.charAt(c2) == Mj) return q(a2, 0, c2);
      c2--;
    }
    return zf;
  }
  function b(b2, c2) {
    while (c2.length > 0) {
      var d2 = b2.stateNames.indexOf(c2);
      if (d2 >= 0) return b2.stateRules[d2];
      c2 = a(c2);
    }
    c2 = b2.stateNames.indexOf(pf);
    return c2 >= 0 ? b2.stateRules[c2] : [];
  }
  function c(a2, b2) {
    return 0 == b2.length ? b2 : b2.indexOf(Mj) >= 0 || 0 == a2.tokenPostfix.length ? b2 : b2 + a2.tokenPostfix;
  }
  return function(a2, d2, e) {
    var g, n2, k2, p2, j2, o2, m2 = [], i2 = 0, h = true, l2 = 0, f = -1;
    while ((h || i2 < e.length) && l2 < 1e4) {
      l2 = l2 + 1 | 0;
      if (!h && i2 == f) {
        f = i2 + 1 | 0;
        if (f >= e.length) break;
      } else {
        f = i2;
      }
      0 == d2[0].length && d2[0].push(pf);
      for (n2 = d2[0][d2[0].length - 1] || "", k2 = b(a2, n2), p2 = q(e, f, e.length), j2 = null, g = zf, i2 = 0; i2 < k2.length; i2++) {
        h = k2[i2];
        if (h.lineStart && 0 != f) continue;
        if (o2 = h.pattern.exec(p2)) {
          g = o2[0] + "", j2 = h;
          break;
        }
      }
      i2 = a2.defaultToken;
      k2 = zf;
      if (j2) {
        h = j2.kind, i2 = j2.token, k2 = j2.next;
      } else {
        if (f < e.length) {
          g = e.charAt(f);
        } else {
          break;
        }
        h = 0;
      }
      if (0 == g.length) {
        if (f < e.length) {
          g = e.charAt(f), i2 = a2.defaultToken;
        } else {
          break;
        }
        h = 0;
      }
      4 == h && (i2 = a2.keywords.indexOf(g) >= 0 ? rh : Nf);
      6 == h ? i2 = f : "@rematch" != i2 ? (j2 = c(a2, i2), j2.length > 0 && (i2 = { offset: 0, type: "" }, i2.offset = f, i2.type = j2, m2.push(i2)), i2 = f + g.length | 0) : i2 = f, 1 == h ? d2[0].length < a2.maxStack && d2[0].push(k2) : 2 == h ? d2[0].length > 1 && d2[0].splice(d2[0].length - 1, 1) : 3 == h ? d2[0][d2[0].length - 1] = k2 : 5 == h ? d2[0] = [pf] : 7 == h && d2[0].length < a2.maxStack && d2[0].push(n2);
      if (0 == i2 && 0 == g.length && 0 == h) break;
      h = false;
    }
    return m2;
  };
})();
function ob(a, g) {
  var h = [], j2 = [[]];
  j2[0] = [pf];
  var d2, e, k2, f, i2, b = 0, c = 0;
  while (c <= g.length) {
    d2 = c == g.length, i2 = !d2 && g.charAt(c) == xf;
    if (d2 || i2) {
      for (e = Ia(a, j2, q(g, b, c)), d2 = 0; d2 < e.length; d2++) k2 = e[d2].type, f = { offset: 0, type: "" }, f.offset = e[d2].offset + b | 0, f.type = k2, h.push(f);
      if (i2) {
        b = c + 1 | 0;
      } else {
        break;
      }
      c = b;
    } else {
      c = c + 1 | 0;
    }
  }
  return h;
}
function j(a, b, c, d2, e, f) {
  a.id = b, a.lexer = c, a.lineComment = d2, a.blockCommentStart = e, a.blockCommentEnd = f, a.tokensProvider = void 0;
}
function k(a) {
  var c = 0;
  while (c < F.length) {
    if (F[c].id == a.id) {
      F[c] = a;
      return;
    }
    c++;
  }
  F.push(a);
}
var aa = /* @__PURE__ */ (function() {
  function a() {
    return "delimiter";
  }
  function b() {
    let b2 = [Bf, Ef, Of, sf, nf, Xf, $f, xk, yf, xh, Rg, Ff, fg, Kf, of, uf, Rf, sg, wf, gj, pg, vf, qg, vh, Ig, dh, tg, qf, fh, Dh, mh, hg, "symbol", Fi, wg, Gf, Jg, Og, Rh, gh, bk, Cf, vg, zg, ih, jh, vi, Mf, Uf, Sf, Af, Tf, ng, Dg, gg, "readonly", Bg, Df, og, "asserts", "keyof", "infer", "never", "unknown", "any", "boolean", Hf, rf, "unique"], a2 = { languageId: "", tokenPostfix: "", defaultToken: "", keywords: [], stateNames: [], stateRules: [], maxStack: 0 };
    S(a2, Jj, ".js", Ui, b2), w(a2, pf), c(a2);
    return a2;
  }
  function c(b2) {
    let c2 = false;
    i(b2, pf, T(), 0, zf, zf, c2), i(b2, pf, "//.*", 0, If, zf, c2), i(b2, pf, Oj, 1, If, If, c2), i(b2, pf, Mh, 1, rf, rf, c2), i(b2, pf, Yi, 1, rf, si, c2);
    let n2 = "`", j2 = "stringT";
    i(b2, pf, n2, 1, rf, j2, c2), i(b2, pf, "0[xX][0-9a-fA-F]+", 0, Hf, zf, c2), i(b2, pf, "\\d+\\.\\d+([eE][+\\-]?\\d+)?", 0, Hf, zf, c2), i(b2, pf, Tj, 0, Hf, zf, c2), i(b2, pf, "[a-zA-Z_$][\\w$]*", 4, Nf, zf, c2), i(b2, pf, Wi, 0, dg, zf, c2), i(b2, pf, "[;,.]", 0, a(), zf, c2), i(b2, pf, "[+\\-*/%&|^~<>=!?:]+", 0, a(), zf, c2), w(b2, If), i(b2, If, Sj, 2, If, zf, c2), i(b2, If, pj, 0, If, zf, c2), i(b2, If, "\\*", 0, If, zf, c2), w(b2, rf), i(b2, rf, qj, 0, Qg, zf, c2), i(b2, rf, Mh, 2, rf, zf, c2), i(b2, rf, lg, 0, rf, zf, c2), w(b2, si), i(b2, si, qj, 0, Qg, zf, c2), i(b2, si, Yi, 2, rf, zf, c2), i(b2, si, fk, 0, rf, zf, c2), w(b2, j2), i(b2, j2, qj, 0, Qg, zf, c2), i(b2, j2, n2, 2, rf, zf, c2), i(b2, j2, "[^\\\\`]+", 0, rf, zf, c2);
  }
  function d2() {
    let d3 = [Gf, of, tg], b2 = { languageId: "", tokenPostfix: "", defaultToken: "", keywords: [], stateNames: [], stateRules: [], maxStack: 0 };
    S(b2, "json", ".json", zf, d3), w(b2, pf), i(b2, pf, T(), 0, zf, zf, false), i(b2, pf, "[{}\\[\\]]", 0, dg, zf, false), i(b2, pf, "[:,]", 0, a(), zf, false), i(b2, pf, "true|false|null", 4, rh, zf, false), i(b2, pf, "-?\\d+(\\.\\d+)?([eE][+\\-]?\\d+)?", 0, Hf, zf, false), i(b2, pf, Mh, 1, rf, rf, false), w(b2, rf), i(b2, rf, qj, 0, Qg, zf, false), i(b2, rf, Mh, 2, rf, zf, false), i(b2, rf, lg, 0, rf, zf, false);
    return b2;
  }
  function e() {
    let c2 = [nj, Rj, "True", mg, og, eg, ih, jh, Bf, sf, nf, "def", "del", Ci, Ff, Pi, uf, Rf, sg, Qi, pg, vf, qg, Vh, "lambda", "nonlocal", eh, $i, "pass", "raise", qf, Jg, Cf, vg, zg, lh, Ef], b2 = { languageId: "", tokenPostfix: "", defaultToken: "", keywords: [], stateNames: [], stateRules: [], maxStack: 0 };
    S(b2, "python", ".python", zf, c2), w(b2, pf), c2 = false, i(b2, pf, T(), 0, zf, zf, c2), i(b2, pf, "#.*", 0, If, zf, c2);
    let m2 = '\\"\\"\\"', g2 = "tstring";
    i(b2, pf, m2, 1, rf, g2, c2);
    let n2 = "'''", h2 = "tstringS";
    i(b2, pf, n2, 1, rf, h2, c2), i(b2, pf, Mh, 1, rf, rf, c2), i(b2, pf, Yi, 1, rf, si, c2), i(b2, pf, "\\d+\\.\\d+", 0, Hf, zf, c2), i(b2, pf, Tj, 0, Hf, zf, c2), i(b2, pf, ti, 4, Nf, zf, c2), i(b2, pf, Wi, 0, dg, zf, c2), i(b2, pf, "[:;,.=+\\-*/%<>!&|^~]+", 0, a(), zf, c2), w(b2, rf), i(b2, rf, qj, 0, Qg, zf, c2), i(b2, rf, Mh, 2, rf, zf, c2), i(b2, rf, lg, 0, rf, zf, c2), w(b2, si), i(b2, si, qj, 0, Qg, zf, c2), i(b2, si, Yi, 2, rf, zf, c2), i(b2, si, fk, 0, rf, zf, c2), w(b2, g2), i(b2, g2, m2, 2, rf, zf, c2), i(b2, g2, '[^"]+', 0, rf, zf, c2), i(b2, g2, Mh, 0, rf, zf, c2), w(b2, h2), i(b2, h2, n2, 2, rf, zf, c2), i(b2, h2, oj, 0, rf, zf, c2), i(b2, h2, Yi, 0, rf, zf, c2);
    return b2;
  }
  function f() {
    let d3 = "html", e2 = "html head body div span script style link meta title p a ul ol li table tr td th form input button img h1 h2 h3 h4 h5 h6 section article nav footer header main pre code textarea select option".split(" "), b2 = { languageId: "", tokenPostfix: "", defaultToken: "", keywords: [], stateNames: [], stateRules: [], maxStack: 0 };
    S(b2, d3, ".html", zf, e2);
    let g2 = pf;
    w(b2, pf), d3 = false, i(b2, pf, T(), 0, zf, zf, d3);
    let f2 = If;
    i(b2, pf, "<!--", 1, If, If, d3), e2 = "tag", i(b2, pf, "</?[a-zA-Z][\\w:-]*", 1, e2, e2, d3), i(b2, pf, "[^<]+", 0, zf, zf, d3), w(b2, If), i(b2, If, "-->", 2, If, zf, d3), i(b2, If, "[^-]+", 0, If, zf, d3), i(b2, If, "-", 0, If, zf, d3), w(b2, e2), i(b2, e2, "/?>", 2, e2, zf, d3), i(b2, e2, T(), 0, zf, zf, d3), i(b2, e2, "[a-zA-Z_:][\\w:.-]*", 0, "attribute.name", zf, d3), i(b2, e2, "=", 0, a(), zf, d3), f2 = "attribute.value", g2 = "attr", i(b2, e2, Mh, 1, f2, g2, d3);
    let h2 = "attrS";
    i(b2, e2, Yi, 1, f2, h2, d3), w(b2, g2), i(b2, g2, Mh, 2, f2, zf, d3), i(b2, g2, tk, 0, f2, zf, d3), w(b2, h2), i(b2, h2, Yi, 2, f2, zf, d3), i(b2, h2, oj, 0, f2, zf, d3);
    return b2;
  }
  function g() {
    let d3 = ["important", kh, tf, "margin", ri, Oi, Jf, Di, "grid", _f, Kh, ah, "right", "bottom", ci, Ng, "font", Nh, "align", "justify", "content", Yf, Hk, "fixed", mf, zh, lf, rg, "inherit", "initial"], b2 = { languageId: "", tokenPostfix: "", defaultToken: "", keywords: [], stateNames: [], stateRules: [], maxStack: 0 };
    S(b2, "css", ".css", zf, d3), w(b2, pf), d3 = false, i(b2, pf, T(), 0, zf, zf, d3), i(b2, pf, Oj, 1, If, If, d3), i(b2, pf, Mh, 1, rf, rf, d3), i(b2, pf, "#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})\\b", 0, "number.hex", zf, d3), i(b2, pf, "-?\\d+(\\.\\d+)?(px|em|rem|%|vh|vw|pt|ex)?", 0, Hf, zf, d3), i(b2, pf, "[a-zA-Z_-][\\w-]*", 4, Nf, zf, d3), i(b2, pf, "[{}();:]", 0, a(), zf, d3), i(b2, pf, "[.,#>\\[\\]+~*]", 0, a(), zf, d3), w(b2, If), i(b2, If, Sj, 2, If, zf, d3), i(b2, If, pj, 0, If, zf, d3), i(b2, If, "\\*", 0, If, zf, d3), w(b2, rf), i(b2, rf, Mh, 2, rf, zf, d3), i(b2, rf, tk, 0, rf, zf, d3);
    return b2;
  }
  function h() {
    let c2 = [], a2 = { languageId: "", tokenPostfix: "", defaultToken: "", keywords: [], stateNames: [], stateRules: [], maxStack: 0 };
    S(a2, Ck, ".md", zf, c2), w(a2, pf);
    let d3 = rh;
    i(a2, pf, "^#{1,6}[ \\t].*$", 0, rh, zf, true), i(a2, pf, "^\\s*[-*+]\\s+", 0, rh, zf, true);
    let f2 = "`+";
    d3 = "code", i(a2, pf, f2, 1, rf, d3, false), i(a2, pf, "\\*\\*[^*]+\\*\\*", 0, "strong", zf, false), i(a2, pf, "\\*[^*]+\\*", 0, "emphasis", zf, false), i(a2, pf, "\\[[^\\]]+\\]\\([^\\)]+\\)", 0, "string.link", zf, false), i(a2, pf, "[^`*\\[#]+", 0, zf, zf, false), i(a2, pf, Mj, 0, zf, zf, false), w(a2, d3), i(a2, d3, f2, 2, rf, zf, false), i(a2, d3, "[^`]+", 0, rf, zf, false);
    return a2;
  }
  return function() {
    if (Va) return;
    Va = true;
    var l2 = b(), a2 = Vf, c2 = jg, i2 = ig, m2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(m2, Jj, l2, Vf, jg, ig), k(m2), m2 = "typescript", l2 = b(), l2.languageId = m2, l2.tokenPostfix = ".ts";
    var n2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(n2, m2, l2, Vf, jg, ig), k(n2), l2 = d2(), m2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(m2, "json", l2, Vf, jg, ig), k(m2), l2 = e(), a2 = '"""', m2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(m2, "python", l2, Ih, a2, a2), k(m2), l2 = f(), m2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(m2, "html", l2, zf, "<!--", "-->"), k(m2), l2 = g(), m2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(m2, "css", l2, zf, jg, ig), k(m2), c2 = h(), i2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(i2, Ck, c2, zf, zf, zf), k(i2), c2 = b(), i2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(i2, Cg, c2, zf, zf, zf), k(i2);
  };
})();
function J(a) {
  aa();
  var c = 0;
  while (c < F.length) {
    if (F[c].id == a) return F[c];
    c++;
  }
  return null;
}
function Ja(a) {
  a = J(a);
  return !a ? null : a.lexer;
}
function pd() {
  aa();
  var a = [], c = 0;
  while (c < F.length) a.push(F[c].id), c++;
  return a;
}
function mc(a, b) {
  return hostComputeLineDiff(a, b);
}
function pb(f) {
  var b, g, h, i2, c, d2 = [], e = u(f), a = 1;
  while (a <= e) {
    b = n(f, a), g = O(b);
    if (g == b.length) {
      a = a + 1 | 0;
      continue;
    }
    c = a;
    b = a + 1 | 0;
    while (b <= e) {
      h = n(f, b), i2 = O(h);
      if (i2 == h.length) {
        b = b + 1 | 0;
        continue;
      }
      if (i2 > g) {
        c = b, b = b + 1 | 0;
        continue;
      }
      break;
    }
    c > a && (b = { startLine: 0, endLine: 0, collapsed: false }, b.startLine = a, b.endLine = c, b.collapsed = false, d2.push(b));
    a = a + 1 | 0;
  }
  return d2;
}
function la(f, a) {
  var b = f.buffer, d2 = I(b, b.root);
  b = hostGetOffsetAt(f.buffer, a.lineNumber, a.column, l);
  if (b >= d2.length) return null;
  var c = d2.charAt(b), e = "([{", g = ")]}", h = e.indexOf(c);
  if (h >= 0) {
    c = b + 1 | 0, b = 1;
    while (c < d2.length) {
      var i2 = d2.charAt(c);
      if (i2 == e.charAt(h)) {
        b = b + 1 | 0;
      } else {
        if (i2 == g.charAt(h)) {
          if (0 == b - 1 | 0) {
            f = hostGetPositionAt(f.buffer, c, l), f = f.column + 1 | 0, d2 = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 }, z(d2, a.lineNumber, a.column, f.lineNumber, f);
            return d2;
          }
        }
      }
      c = c + 1 | 0;
    }
    return null;
  }
  h = g.indexOf(c);
  if (h >= 0) {
    c = b - 1 | 0, b = 1;
    while (c >= 0) {
      i2 = d2.charAt(c);
      if (i2 == g.charAt(h)) {
        b = b + 1 | 0;
      } else {
        if (i2 == e.charAt(h)) {
          if (0 == b - 1 | 0) {
            f = hostGetPositionAt(f.buffer, c, l), a = a.column + 1 | 0, d2 = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 }, z(d2, f.lineNumber, f.column, a.lineNumber, a);
            return d2;
          }
        }
      }
      c--;
    }
  }
  return null;
}
var qb = /* @__PURE__ */ (function() {
  function a(a2, b2, c, d2) {
    if (0 == a2.length) return;
    if (c.length >= 200) return;
    if (d2.indexOf(a2) >= 0) return;
    (0 == b2.length || 0 == a2.indexOf(b2) && a2 != b2) && (b2 = { label: "", insertText: "", kind: 0, detail: "" }, b2.label = a2, b2.insertText = a2, b2.kind = 18, b2.detail = emptyBuf(), c.push(b2), d2.push(a2));
  }
  function b(e, b2, d2, f) {
    var h, j2, g = emptyBuf(), i2 = e.length, c = 0;
    while (c <= i2) h = emptyBuf(), j2 = c < i2 ? Fa(e.charAt(c)) : false, j2 ? g = g + h : (a(g, b2, d2, f), g = emptyBuf()), c = c + 1 | 0;
  }
  return function(f, c) {
    var h = x(n(f, c.lineNumber), c.column)[0];
    c = [];
    var d2 = [], i2 = u(f), e = 1;
    while (e <= i2 && c.length < 200) b(n(f, e), h, c, d2), e = e + 1 | 0;
    if (f = J(f.languageId)) {
      e = 0;
      while (e < f.lexer.keywords.length) a(f.lexer.keywords[e] || "", h, c, d2), e++;
    }
    return c;
  };
})();
function rb(a) {
  var d2, b = emptyBuf(), c = 0;
  while (c < a.length) {
    if ("$" == a.charAt(c) && c + 1 < a.length) {
      d2 = a.charAt(c + 1);
      if ("0" == d2 || "1" == d2 || "2" == d2 || "3" == d2) {
        c = c + 2 | 0;
        continue;
      }
      if (d2 == Nj) {
        c = c + 2 | 0;
        while (c < a.length && "}" != a.charAt(c)) c = c + 1 | 0;
        c = c + 1 | 0;
        continue;
      }
    }
    b += a.charAt(c);
    c++;
  }
  return b;
}
function sb(f, a, b) {
  b = J(b);
  var c, d2 = Vf;
  b && b.lineComment.length > 0 && (d2 = b.lineComment), b = n(f, a);
  var e = O(b);
  c = q(b, e, b.length), 0 == c.indexOf(d2) ? (d2 = d2.length, c.length > d2 && c.charAt(d2) == Hh && (d2 = d2 + 1 | 0), o(f, [v(a, 1, a, b.length + 1 | 0, q(b, 0, e) + q(c, d2, c.length))], true)) : o(f, [v(a, 1, a, b.length + 1 | 0, q(b, 0, e) + d2 + Hh + c)], true);
}
function nc(h) {
  var d2, e, a = emptyBuf(), b = 0, c = 0;
  while (c < h.length) {
    d2 = h.charAt(c);
    if (d2 == Nj) {
      a = a + d2 + xf, b = b + 1 | 0, d2 = 0;
      while (d2 < b) a += "  ", d2 = d2 + 1 | 0;
    } else {
      if ("}" == d2) {
        b > 0 && (b = b - 1 | 0), a += xf, e = 0;
        while (e < b) a += "  ", e = e + 1 | 0;
        a += d2;
      } else {
        a += d2;
      }
    }
    c++;
  }
  return a;
}
function ma(a, b, d2) {
  var f, e = [], c = 0;
  while (c < G.length && (d2 <= 0 || e.length < d2)) f = G[c], (0 == a.length || f.owner == a) && (0 == b.length || f.resource == b) && e.push(f), c++;
  return e;
}
function oc(a) {
  return a >= Wa ? "squiggly-error" : a >= Nb ? "squiggly-warning" : a >= Mb ? "squiggly-info" : "squiggly-hint";
}
function pc(a, b, c) {
  var d2 = Wa, e = zf;
  if (a) {
    var f = a.startLineNumber ? +a.startLineNumber | 0 : 1, g = a.startColumn ? +a.startColumn | 0 : 1, h = a.endLineNumber ? +a.endLineNumber | 0 : 1, i2 = a.endColumn ? +a.endColumn | 0 : 1;
    !a.severity || (d2 = +a.severity | 0), !a.message || (e = a.message + "");
  } else {
    f = 1, g = 1, h = 1, i2 = 1;
  }
  a = { owner: "", message: "", severity: 0, startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, resource: "" };
  a.owner = b, a.message = e, a.severity = d2, a.startLineNumber = f, a.startColumn = g, a.endLineNumber = h, a.endColumn = i2, a.resource = c;
  return a;
}
function qc(a, b, f) {
  a.root = b, a.model = f, b = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 }, D(b, 1, 1, 1, 1), a.selection = b, a.extraCursors = [], a.scrollTop = 0, a.lineHeight = 19, a.charWidth = 8, a.width = 800, a.height = 400, a.showLineNumbers = true, a.showMinimap = true, a.theme = "vs", a.languageId = f.languageId, a.readOnly = false, a.fontSize = 14, a.tabSize = 4, a.insertSpaces = true, a.wordWrap = false, a.mouseSelecting = false, a.mouseAnchorLine = 1, a.mouseAnchorColumn = 1, a.folds = [], a.collapsed = [], a.findOpen = false, a.findQuery = zf, a.replaceQuery = zf, a.findIndex = 0, a.suggestOpen = false, a.suggestItems = [], a.suggestIndex = 0, a.hoverOpen = false, a.contextOpen = false, a.gotoOpen = false, a.renameOpen = false, a.renameWord = zf, a.modelFacade = void 0, a.overflow = document.createElement(ej), a.margin = document.createElement(ej), a.scrollable = document.createElement(ej), a.linesHost = document.createElement(ej), a.textarea = document.createElement(Jk), a.minimapCanvas = document.createElement("canvas"), a.widgetsHost = document.createElement(ej), a.cursorEl = document.createElement(ej), a.selectionHost = document.createElement(ej), a.findWidget = document.createElement(ej), a.findInput = document.createElement(_h), a.replaceInput = document.createElement(_h), a.suggestWidget = document.createElement(ej), a.hoverWidget = document.createElement(ej), a.contextWidget = document.createElement(ej), a.gotoWidget = document.createElement(ej), a.gotoInput = document.createElement(_h), a.renameWidget = document.createElement(ej), a.renameInput = document.createElement(_h), a.paramWidget = document.createElement(ej), a.stickyWidget = document.createElement(ej), rc(a);
}
function rc(a) {
  setClassName(a.root, Gi + a.theme);
  var d2 = Hk;
  setStyle(a.root, _f, Hk);
  var e = "overflow";
  setStyle(a.root, e, "hidden"), setStyle(a.root, "font-family", "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace");
  var b = bh;
  setStyle(a.root, Qh, a.fontSize.toString(10) + bh), setStyle(a.root, "line-height", a.lineHeight.toString(10) + bh), setClassName(a.overflow, "overflow-guard"), setStyle(a.overflow, Jf, Di), setStyle(a.overflow, Ng, "100%"), setClassName(a.margin, "margin"), setStyle(a.margin, ci, "56px"), setStyle(a.margin, Di, "0 0 56px"), setStyle(a.margin, "user-select", lf), setClassName(a.scrollable, "monaco-scrollable-element"), setStyle(a.scrollable, Di, "1 1 auto"), setStyle(a.scrollable, e, rg), setStyle(a.scrollable, _f, Hk), setClassName(a.linesHost, "view-lines"), a.wordWrap ? (setStyle(a.linesHost, bg, Fk), a.textarea.setAttribute(Ph, "soft")) : (setStyle(a.linesHost, bg, "pre"), a.textarea.setAttribute(Ph, ki)), setStyle(a.selectionHost, _f, Yf);
  var i2 = "inset";
  d2 = "0", setStyle(a.selectionHost, i2, d2), setStyle(a.selectionHost, Eg, lf), setClassName(a.cursorEl, "cursor"), setStyle(a.cursorEl, _f, Yf), setStyle(a.cursorEl, ci, "2px"), setStyle(a.cursorEl, Ng, a.lineHeight.toString(10) + bh), setStyle(a.cursorEl, tf, "#aeafad"), setStyle(a.cursorEl, Eg, lf), a.textarea.setAttribute(sf, "inputarea");
  var h = ki;
  a.textarea.setAttribute(Ph, ki), a.textarea.setAttribute("autocorrect", ki), a.textarea.setAttribute("autocapitalize", ki), a.textarea.setAttribute("spellcheck", of), setTabIndex(a.textarea, 0), setStyle(a.textarea, _f, Yf), h = "1px", setStyle(a.textarea, ci, h), setStyle(a.textarea, Ng, h), setStyle(a.textarea, "opacity", d2), setClassName(a.minimapCanvas, qi), setStyle(a.minimapCanvas, ci, "64px"), setStyle(a.minimapCanvas, Di, "0 0 64px"), setClassName(a.widgetsHost, "overflowingContentWidgets"), setStyle(a.widgetsHost, _f, Yf), setStyle(a.widgetsHost, i2, d2), setStyle(a.widgetsHost, Eg, lf), sc(a), U(a, a.suggestWidget, "suggest-widget"), U(a, a.hoverWidget, "monaco-hover"), U(a, a.contextWidget, "context-view"), U(a, a.gotoWidget, "goto-line-widget"), setPlaceholder(a.gotoInput, "Go to line"), a.gotoWidget.appendChild(a.gotoInput), U(a, a.renameWidget, "rename-box"), a.renameWidget.appendChild(a.renameInput), U(a, a.paramWidget, "parameter-hints-widget"), U(a, a.stickyWidget, "sticky-widget"), a.scrollable.appendChild(a.selectionHost), a.scrollable.appendChild(a.linesHost), a.scrollable.appendChild(a.cursorEl), a.overflow.appendChild(a.margin), a.overflow.appendChild(a.scrollable), a.overflow.appendChild(a.minimapCanvas), a.root.appendChild(a.overflow), a.root.appendChild(a.textarea), a.root.appendChild(a.widgetsHost), ba(a), setDisplay(a.findWidget, lf), setDisplay(a.suggestWidget, lf), setDisplay(a.hoverWidget, lf), setDisplay(a.contextWidget, lf), setDisplay(a.gotoWidget, lf), setDisplay(a.renameWidget, lf), setDisplay(a.paramWidget, lf), b = document.createElement("style"), setTextContent(b, ".mtk-keyword{color:#569cd6}.mtk-string{color:#ce9178}.mtk-comment{color:#6a9955}.mtk-number{color:#b5cea8}.mtk-tag{color:#569cd6}.mtk-attr{color:#9cdcfe}.squiggly-error{text-decoration:underline wavy #f14c4c}.squiggly-warning{text-decoration:underline wavy #cca700}.squiggly-info{text-decoration:underline wavy #3794ff}.squiggly-hint{text-decoration:underline dotted #eeeeee}.folding{cursor:pointer;display:inline-block;width:12px}"), a.root.appendChild(b);
}
function U(a, b, c) {
  setClassName(b, c), setStyle(b, _f, Yf), setStyle(b, Eg, rg), setStyle(b, "z-index", "40"), setStyle(b, Jf, lf), setStyle(b, "max-width", "420px"), setStyle(b, ri, "6px 8px"), setStyle(b, Oi, fi), setStyle(b, tf, "#252526"), setStyle(b, kh, "#cccccc"), setStyle(b, Qh, "12px"), a.widgetsHost.appendChild(b);
}
function sc(a) {
  setClassName(a.findWidget, "editor-widget find-widget"), setStyle(a.findWidget, _f, Yf);
  let b = "8px";
  setStyle(a.findWidget, Kh, b), setStyle(a.findWidget, "right", "72px"), setStyle(a.findWidget, "z-index", "50"), setStyle(a.findWidget, Eg, rg), setStyle(a.findWidget, Jf, lf), setStyle(a.findWidget, tf, "#252526"), setStyle(a.findWidget, Oi, fi), setStyle(a.findWidget, ri, b), setPlaceholder(a.findInput, "Find"), setPlaceholder(a.replaceInput, "Replace"), b = "data-monaco", a.findInput.setAttribute(b, "find"), a.replaceInput.setAttribute(b, "replace"), a.findWidget.appendChild(a.findInput), a.findWidget.appendChild(a.replaceInput), a.widgetsHost.appendChild(a.findWidget);
}
function ba(a) {
  var d2 = a.root, f = a.theme;
  setClassName(d2, Gi + f), setStyle(d2, Qh, a.fontSize.toString(10) + bh);
  if ("vs-dark" == f || f == Ak) {
    setStyle(d2, tf, "#1e1e1e"), setStyle(d2, kh, "#d4d4d4"), setStyle(a.cursorEl, tf, "#aeafad");
  } else {
    setStyle(d2, tf, "#fffffe");
    var c = "#000000";
    setStyle(d2, kh, c), setStyle(a.cursorEl, tf, c);
  }
  a.wordWrap ? (setStyle(a.linesHost, bg, Fk), a.textarea.setAttribute(Ph, "soft")) : (setStyle(a.linesHost, bg, "pre"), a.textarea.setAttribute(Ph, ki));
}
function tb(a, b, c) {
  a.width = b, a.height = c, setStyle(a.root, ci, b.toString(10) + bh), setStyle(a.root, Ng, c.toString(10) + bh), m(a);
}
function Ka(a) {
  var e = (a.scrollTop / a.lineHeight | 0) + 1 | 0;
  return e < 1 ? 1 : oa(a, e);
}
function ub(a) {
  var b = (Ka(a) + (a.height / a.lineHeight | 0) | 0) + 8 | 0;
  a = u(a.model);
  return b > a || b;
}
function vb(a) {
  var c = u(a.model), b = 0, e = 1;
  while (e <= c) na(a, e) || (b = b + 1 | 0), e = e + 1 | 0;
  return b;
}
function na(a, e) {
  var b, c = 0;
  while (c < a.folds.length) {
    b = a.folds[c];
    if (b.collapsed && e > b.startLine && e <= b.endLine) return true;
    c++;
  }
  return false;
}
function oa(a, b) {
  var c = u(a.model);
  while (b <= c && na(a, b)) b = b + 1 | 0;
  return b > c || b;
}
function pa(a, e) {
  var c = 0;
  while (c < a.folds.length) {
    if (a.folds[c].startLine == e) return a.folds[c];
    c++;
  }
  return null;
}
function La(a) {
  var d2, b = pb(a.model), c = 0;
  while (c < b.length) d2 = pa(a, b[c].startLine), d2 && (b[c].collapsed = d2.collapsed), c++;
  a.folds = b;
}
function qa(a, e) {
  var b = pa(a, e);
  b || (La(a), b = pa(a, e));
  if (!b) return;
  b.collapsed = !b.collapsed, m(a);
}
function wb(a, b) {
  La(a);
  var c = 0;
  while (c < a.folds.length) a.folds[c].collapsed = b, c++;
  m(a);
}
function tc(a) {
  return a.indexOf(If) >= 0 ? "mtk-comment" : a.indexOf(rf) >= 0 ? "mtk-string" : a.indexOf(rh) >= 0 ? "mtk-keyword" : a.indexOf(Hf) >= 0 ? "mtk-number" : a.indexOf("tag") >= 0 ? "mtk-tag" : a.indexOf(hk) >= 0 ? "mtk-attr" : "mtk";
}
function xb(a) {
  var d2, b = emptyBuf(), c = 0;
  while (c < a.length) d2 = a.charAt(c), "<" == d2 ? b = b + "&lt;" : ">" == d2 ? b = b + "&gt;" : "&" == d2 ? b = b + "&amp;" : b += d2, c++;
  return b;
}
function uc(a, b) {
  if (0 == b.length) {
    return 0 == a.length ? "<span>&nbsp;</span>" : '<span class="mtk">' + xb(a) + "</span>";
  }
  var f, e, d2 = emptyBuf(), c = 0;
  while (c < b.length) f = b[c].offset, e = a.length, c + 1 < b.length && (e = b[c + 1].offset), e = q(a, f, e), d2 += '<span class="', d2 = d2 + tc(b[c].type) + dj + xb(e) + "</span>", c++;
  return d2;
}
function vc(a) {
  if (!a.showMinimap) {
    setStyle(a.minimapCanvas, Jf, lf);
    return;
  }
  setStyle(a.minimapCanvas, Jf, mf);
  var c = a.height;
  c < 1 && (c = 1);
  var r2 = a.minimapCanvas;
  canvasSetSize(r2, 64, c);
  var e = canvasGetContext2d(r2), b = "#f3f3f3", f = "#6e6e6e";
  ("vs-dark" == a.theme || a.theme == Ak) && (b = "#1e1e1e", f = "#5a5a5a"), canvasFillRect(e, 0, 0, 64, c, b);
  var d2 = u(a.model);
  if (d2 < 1) return;
  b = c / d2, b < 1 && (b = 1), c = 1;
  while (c <= d2) {
    var h, g = t(a.model, c);
    g > 0 && !na(a, c) && (h = (c - 1 | 0) * b, canvasFillRect(e, 2, h, Cc(g), b, f)), c = c + 1 | 0;
  }
}
function wc(a, e, c) {
  var b = emptyBuf(), d2 = a.model, f = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
  z(f, e, 1, e, c), c = d2.decorations, d2 = [], Da(c, c.root, f, d2), c = 0;
  while (c < d2.length) b = b + Hh + d2[c].className, c++;
  d2 = ma(zf, X(a.model.uri), 0), a = 0;
  while (a < d2.length) c = d2[a], c.startLineNumber == e && (b = b + Hh, b = b + oc(c.severity)), a++;
  return b;
}
function xc(a, b) {
  var d2 = [];
  a = hostCall(a, "tokenize", b, void 0, void 0), a && a.tokens && (a = a.tokens);
  var f, g, e = jsArrayLen(a), c = 0;
  while (c < e) b = jsArrayAt(a, c), f = jsPropInt(b, "offset", 0), g = jsPropString(b, Mf, jsPropString(b, "scopes", "mtk")), b = { offset: 0, type: "" }, b.offset = f, b.type = g, d2.push(b), c++;
  return d2;
}
function yc(a, e, b) {
  return !a.showLineNumbers ? emptyBuf() : '<div class="line-number" data-line="' + e.toString(10) + Th + a.lineHeight.toString(10) + 'px"><span class="folding" data-fold="' + e.toString(10) + dj + b + "</span> " + e.toString(10) + Kg;
}
function zc(a, e) {
  a = pa(a, e);
  return !a ? emptyBuf() : a.collapsed ? "\u25B6" : "\u25BC";
}
function Ac(a, b, c, d2) {
  var f = [];
  return a && a.tokensProvider ? xc(a.tokensProvider, d2) : b ? Ia(b, c, d2) : f;
}
function m(a) {
  ba(a), La(a);
  var e = Ka(a), h = ub(a), i2 = J(a.languageId), d2 = Ja(a.languageId), g = [[]];
  g[0] = [pf];
  var c, f, b = 1;
  while (b < e && d2) Ia(d2, g, n(a.model, b)), b = b + 1 | 0;
  b = emptyBuf(), c = emptyBuf();
  while (e <= h) {
    if (na(a, e)) {
      e = e + 1 | 0;
      continue;
    }
    f = n(a.model, e);
    b += yc(a, e, zc(a, e)), f = '<div class="view-line' + wc(a, e, f.length + 1 | 0) + '" data-line="' + e.toString(10) + Th + a.lineHeight.toString(10) + 'px">' + uc(f, Ac(i2, d2, g, f)) + Kg, c += f, e = e + 1 | 0;
  }
  setInnerHTML(a.margin, b);
  setInnerHTML(a.linesHost, c), setStyle(a.linesHost, Ng, ((vb(a) * a.lineHeight | 0) + (a.height / 2 | 0) | 0).toString(10) + bh), vc(a), Bc(a), Ma(a);
}
function yb(a, e, b, c) {
  e = (e - 1 | 0) * a.lineHeight | 0;
  let d2 = (b - 1 | 0) * a.charWidth | 0;
  return '<div class="selected-text" style="position:absolute;top:' + e.toString(10) + "px;left:" + d2.toString(10) + "px;width:" + zb(1, (c - b | 0) * a.charWidth | 0).toString(10) + "px;height:" + a.lineHeight.toString(10) + 'px;background:rgba(38,79,120,0.45)"></div>';
}
function Bc(a) {
  var e, d2, b = a.selection, c = emptyBuf();
  if (b.startLineNumber != b.endLineNumber || b.startColumn != b.endColumn) {
    e = b.startLineNumber;
    while (e <= b.endLineNumber) d2 = t(a.model, e) + 1 | 0, e == b.endLineNumber && (d2 = b.endColumn), c += yb(a, e, e == e ? b.startColumn : 1, d2), e = e + 1 | 0;
  }
  e = 0;
  while (e < a.extraCursors.length) b = a.extraCursors[e], c += yb(a, b.startLineNumber, b.startColumn, b.endColumn), e++;
  setInnerHTML(a.selectionHost, c);
}
function Ma(a) {
  let c = (a.selection.positionLineNumber - 1 | 0) * a.lineHeight | 0, d2 = (a.selection.positionColumn - 1 | 0) * a.charWidth | 0;
  setStyle(a.textarea, Kh, c.toString(10) + bh), setStyle(a.textarea, ah, (d2 + 56 | 0).toString(10) + bh), setStyle(a.cursorEl, Kh, c.toString(10) + bh), setStyle(a.cursorEl, ah, d2.toString(10) + bh), inputSetValue(a.textarea, zf);
}
function ra(a, e, b) {
  var d2 = e - rectLeft(a.scrollable) | 0;
  e = (((b - rectTop(a.scrollable) | 0) + a.scrollTop | 0) / a.lineHeight | 0) + 1 | 0, e < 1 && (e = 1), b = u(a.model), e > b || (b = e), b = oa(a, b), e = (d2 / a.charWidth | 0) + 1 | 0, a = t(a.model, b) + 1 | 0, e < 1 && (e = 1), e > a || (a = e), e = { lineNumber: 0, column: 0 }, e.lineNumber = b, e.column = a;
  return e;
}
function sa(a, b) {
  b = (b - 1 | 0) * a.lineHeight | 0;
  var c = a.scrollTop + a.height | 0;
  (b < a.scrollTop || (b + a.lineHeight | 0) > c) && (a.scrollTop = zb(0, b - (a.height / 3 | 0) | 0));
}
function Cc(a) {
  return a < 60 ? a : 60;
}
function zb(a, b) {
  return a > b ? a : b;
}
function Dc(a, f) {
  let b = { root: null, overflow: null, margin: null, scrollable: null, linesHost: null, textarea: null, minimapCanvas: null, widgetsHost: null, cursorEl: null, selectionHost: null, findWidget: null, findInput: null, replaceInput: null, suggestWidget: null, hoverWidget: null, contextWidget: null, gotoWidget: null, gotoInput: null, renameWidget: null, renameInput: null, paramWidget: null, stickyWidget: null, model: null, selection: null, extraCursors: [], scrollTop: 0, lineHeight: 0, charWidth: 0, width: 0, height: 0, showLineNumbers: false, showMinimap: false, theme: "", languageId: "", readOnly: false, fontSize: 0, tabSize: 0, insertSpaces: false, wordWrap: false, mouseSelecting: false, mouseAnchorLine: 0, mouseAnchorColumn: 0, folds: [], collapsed: [], findOpen: false, findQuery: "", replaceQuery: "", findIndex: 0, suggestOpen: false, suggestItems: [], suggestIndex: 0, hoverOpen: false, contextOpen: false, gotoOpen: false, renameOpen: false, renameWord: "", modelFacade: null };
  qc(b, a, f);
  return b;
}
function Y(b) {
  var a = u(b.model), e = b.selection.positionLineNumber;
  e < 1 && (e = 1), e > a && (e = a);
  var f = t(b.model, e) + 1 | 0, c = b.selection.positionColumn;
  c < 1 && (c = 1), c > f || (f = c), c = b.selection.selectionStartLineNumber;
  var d2 = b.selection.selectionStartColumn;
  c < 1 && (c = 1), c > a || (a = c), c = t(b.model, a) + 1 | 0, d2 < 1 && (d2 = 1), d2 > c || (c = d2), d2 = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 }, D(d2, a, c, e, f), b.selection = d2;
}
function p(b, e, a) {
  let c = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 };
  D(c, e, a, e, a), b.selection = c, Y(b), sa(b, b.selection.positionLineNumber), Ma(b);
}
function s(b, a, c, d2, e) {
  let f = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 };
  D(f, a, c, d2, e), b.selection = f, Y(b), sa(b, b.selection.positionLineNumber), Ma(b);
}
function K(b, g) {
  let a = b.selection, c = a.startLineNumber, d2 = a.startColumn;
  o(b.model, [v(c, d2, a.endLineNumber, a.endColumn, g)], true), g = hostGetPositionAt(b.model.buffer, hostGetOffsetAt(b.model.buffer, c, d2, l) + g.length | 0, l), p(b, g.lineNumber, g.column);
}
function Ab(a) {
  var b = emptyBuf(), c = 0;
  while (c < a.length) c && (b = b + xf), b += a[c] || "", c++;
  return b;
}
function Ec(a) {
  return "(" == a ? ")" : "[" == a ? "]" : a == Nj ? "}" : '"' == a || a == Yi ? a : zf;
}
function ca(b, g) {
  if (b.readOnly || 0 == g.length) return;
  if (1 == g.length) {
    var a = Ec(g);
    if (a.length > 0) {
      g += a, K(b, g), P(b, g), ta(b, false), m(b);
      return;
    }
  }
  K(b, g);
  P(b, g), m(b);
}
function P(b, g) {
  var c = b.extraCursors.length;
  if (0 == c) return;
  c--;
  while (c >= 0) {
    var a = b.extraCursors[c], d2 = a.startLineNumber, e = a.startColumn, f = b.model;
    o(f, [v(d2, e, a.endLineNumber, a.endColumn, g)], true), d2 = hostGetPositionAt(f.buffer, hostGetOffsetAt(f.buffer, d2, e, l) + g.length | 0, l), a = d2.lineNumber, d2 = d2.column, e = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 }, D(e, a, d2, a, d2), b.extraCursors[c] = e, c--;
  }
}
function Fc(b) {
  var c = b.extraCursors.length;
  if (0 == c) return;
  c--;
  while (c >= 0) {
    var e = b.extraCursors[c], a = e.positionLineNumber, d2 = e.positionColumn;
    if (1 == a && 1 == d2) {
      c--;
      continue;
    }
    var f = hostGetPositionAt(b.model.buffer, hostGetOffsetAt(b.model.buffer, a, d2, l) - 1 | 0, l);
    e = f.lineNumber, f = f.column, o(b.model, [Q(e, f, a, d2)], true), a = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 }, D(a, e, f, e, f), b.extraCursors[c] = a, c--;
  }
}
function Gc(b) {
  var c = b.extraCursors.length;
  if (0 == c) return;
  c--;
  while (c >= 0) {
    var d2 = b.extraCursors[c], a = d2.positionLineNumber;
    d2 = d2.positionColumn;
    var e = hostGetOffsetAt(b.model.buffer, a, d2, l);
    if (e >= b.model.buffer.length) {
      c--;
      continue;
    }
    e = hostGetPositionAt(b.model.buffer, e + 1 | 0, l);
    o(b.model, [Q(a, d2, e.lineNumber, e.column)], true), e = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 }, D(e, a, d2, a, d2), b.extraCursors[c] = e, c--;
  }
}
function ta(b, a) {
  var c = hostGetOffsetAt(b.model.buffer, b.selection.positionLineNumber, b.selection.positionColumn, l);
  c > 0 && (c = c - 1 | 0), c = hostGetPositionAt(b.model.buffer, c, l), a ? s(b, b.selection.selectionStartLineNumber, b.selection.selectionStartColumn, c.lineNumber, c.column) : p(b, c.lineNumber, c.column);
}
function Hc(b) {
  var a = b.selection.endLineNumber;
  a < u(b.model) && (a = a + 1 | 0);
  var c = a == b.selection.endLineNumber ? t(b.model, a) + 1 | 0 : 1;
  s(b, b.selection.startLineNumber, 1, a, c);
}
function V(b) {
  let a = b.selection, c = a.startLineNumber, d2 = a.startColumn, e = a.endLineNumber;
  let f = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
  z(f, c, d2, e, a.endColumn);
  return Aa(b.model.buffer, f);
}
function Na(b) {
  var g = V(b);
  0 == g.length && (g = n(b.model, b.selection.positionLineNumber) + xf), clipboardWrite(g);
}
function Oa(b) {
  Na(b);
  if (b.readOnly) return;
  var e;
  b.selection.startLineNumber == b.selection.endLineNumber && b.selection.startColumn == b.selection.endColumn ? (e = b.selection.positionLineNumber, e < u(b.model) ? o(b.model, [v(e, 1, e + 1 | 0, 1, zf)], true) : o(b.model, [v(e, 1, e, t(b.model, e) + 1 | 0, zf)], true), p(b, e, 1)) : K(b, zf), m(b);
}
function Pa(b, a) {
  if (b.readOnly) return;
  0 == a.length && (a = clipboardRead());
  if (0 == a.length) return;
  K(b, a), P(b, a), m(b);
}
function Ic(b) {
  if (b.readOnly) return;
  var c = b.selection.startLineNumber, a = b.selection.endLineNumber;
  if (a < u(b.model)) {
    o(b.model, [v(c, 1, a + 1 | 0, 1, zf)], true);
  } else {
    if (c > 1) {
      d = c - 1 | 0;
      var e = t(b.model, d) + 1 | 0, f = b.model;
      o(f, [Q(d, e, a, t(f, a) + 1 | 0)], true);
    } else {
      o(b.model, [v(1, 1, a, t(b.model, a) + 1 | 0, zf)], true);
    }
  }
  p(b, c, 1);
  m(b);
}
function Bb(b, a) {
  if (b.readOnly) return;
  var d2 = b.selection.startLineNumber, c = b.selection.endLineNumber, f = [], e = d2;
  while (d2 <= c) f.push(n(b.model, d2)), e = e + 1 | 0;
  e = Ab(f), a < 0 ? (a = b.model, o(a, [v(d2, 1, d2, 1, e + xf)], true)) : (a = t(b.model, c) + 1 | 0, f = b.model, o(f, [v(c, a, c, a, xf + e)], true), a = (c - d2 | 0) + 1 | 0, d2 = d2 + a | 0, a = c + a | 0, s(b, d2, 1, a, t(f, a) + 1 | 0)), m(b);
}
function Cb(b, e) {
  var a = b.selection;
  e = a.positionLineNumber + e | 0;
  var c;
  if (e < 1 || e > u(b.model)) return;
  c = t(b.model, e) + 1 | 0, a.positionColumn > c && (a = c), c = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 }, D(c, e, a, e, a), b.extraCursors.push(c);
}
function Jc(b) {
  if (b.readOnly) return;
  var c, a, d2, f = u(b.model), e = 1;
  while (e <= f) {
    c = n(b.model, e), a = c.length;
    while (a > 0) {
      d2 = c.charAt(a - 1);
      if (d2 != Hh && d2 != _i) break;
      a--;
    }
    a < a && (d2 = b.model, o(d2, [Q(e, a + 1 | 0, e, a + 1 | 0)], true));
    e = e + 1 | 0;
  }
  m(b);
}
function Kc(b) {
  if (b.readOnly) return;
  var e = b.selection.endLineNumber, a = t(b.model, e) + 1 | 0, c = b.model;
  o(c, [v(e, a, e, a, xf)], true), p(b, e + 1 | 0, 1), m(b);
}
function Lc(b) {
  if (b.readOnly) return;
  var e = b.selection.startLineNumber, a = b.model;
  o(a, [v(e, 1, e, 1, xf)], true), p(b, e, 1), m(b);
}
function Mc(b) {
  if (b.readOnly) return;
  var e = b.selection.startLineNumber;
  if (e >= u(b.model)) return;
  var a = n(b.model, e), c = n(b.model, e + 1 | 0), d2 = q(c, O(c), c.length);
  d2 = a.length > 0 && d2.length > 0 ? a + Hh + d2 : a + d2, o(b.model, [v(e, 1, e + 1 | 0, c.length + 1 | 0, d2)], true), p(b, e, a.length + 1 | 0), m(b);
}
function Nc(b) {
  let a = x(n(b.model, b.selection.positionLineNumber), b.selection.positionColumn);
  s(b, b.selection.positionLineNumber, a[1], b.selection.positionLineNumber, a[2]);
}
var Oc;
var Pc;
(function() {
  function a(b2) {
    if (b2.readOnly) return;
    var c2, d3, e2, a2 = b2.selection;
    if (a2.startLineNumber != a2.endLineNumber || a2.startColumn != a2.endColumn) {
      K(b2, zf), P(b2, zf), m(b2);
      return;
    }
    if (1 == a2.positionLineNumber && 1 == a2.positionColumn) return;
    c2 = a2.positionLineNumber, a2 = a2.positionColumn, e2 = hostGetPositionAt(b2.model.buffer, hostGetOffsetAt(b2.model.buffer, c2, a2, l) - 1 | 0, l), d3 = e2.lineNumber, e2 = e2.column, o(b2.model, [Q(d3, e2, c2, a2)], true), p(b2, d3, e2), Fc(b2), m(b2);
  }
  function b(b2) {
    if (b2.readOnly) return;
    var c2, d3, a2 = b2.selection;
    if (a2.startLineNumber != a2.endLineNumber || a2.startColumn != a2.endColumn) {
      K(b2, zf), P(b2, zf), m(b2);
      return;
    }
    c2 = a2.positionLineNumber;
    a2 = a2.positionColumn, d3 = hostGetOffsetAt(b2.model.buffer, c2, a2, l);
    if (d3 >= b2.model.buffer.length) return;
    d3 = hostGetPositionAt(b2.model.buffer, d3 + 1 | 0, l), o(b2.model, [Q(c2, a2, d3.lineNumber, d3.column)], true), Gc(b2), m(b2);
  }
  function c(b2, a2) {
    var c2 = hostGetOffsetAt(b2.model.buffer, b2.selection.positionLineNumber, b2.selection.positionColumn, l);
    c2 < b2.model.buffer.length && (c2 = c2 + 1 | 0), c2 = hostGetPositionAt(b2.model.buffer, c2, l), a2 ? s(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, c2.lineNumber, c2.column) : p(b2, c2.lineNumber, c2.column);
  }
  function d2(b2, a2) {
    var e2 = b2.selection.positionLineNumber - 1 | 0;
    e2 < 1 && (e2 = 1);
    var c2 = oa(b2, e2);
    e2 = b2.selection.positionColumn;
    var d3 = t(b2.model, c2) + 1 | 0;
    e2 > d3 && (e2 = d3), a2 ? s(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, c2, e2) : p(b2, c2, e2);
  }
  function e(b2, a2) {
    var e2 = b2.selection.positionLineNumber + 1 | 0, c2 = u(b2.model);
    e2 > c2 && (e2 = c2), c2 = oa(b2, e2), e2 = b2.selection.positionColumn;
    var d3 = t(b2.model, c2) + 1 | 0;
    e2 > d3 && (e2 = d3), a2 ? s(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, c2, e2) : p(b2, c2, e2);
  }
  function f(b2, a2) {
    var e2 = b2.selection.positionLineNumber, d3 = n(b2.model, e2), c2 = O(d3);
    c2 = b2.selection.positionColumn != (c2 + 1 | 0) && c2 < d3.length ? c2 + 1 | 0 : 1, a2 ? s(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, e2, c2) : p(b2, e2, c2);
  }
  function g(b2, a2) {
    var e2 = b2.selection.positionLineNumber, c2 = t(b2.model, e2) + 1 | 0;
    a2 ? s(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, e2, c2) : p(b2, e2, c2);
  }
  function h(b2, a2) {
    a2 ? s(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, 1, 1) : p(b2, 1, 1);
  }
  function i2(b2, a2) {
    var c2 = u(b2.model), d3 = t(b2.model, c2) + 1 | 0;
    a2 ? s(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, c2, d3) : p(b2, c2, d3);
  }
  function j2(b2, a2) {
    var e2 = b2.height / b2.lineHeight | 0;
    e2 < 1 && (e2 = 1), e2 = b2.selection.positionLineNumber - e2 | 0, e2 < 1 && (e2 = 1), a2 ? s(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, e2, b2.selection.positionColumn) : p(b2, e2, b2.selection.positionColumn);
  }
  function k2(b2, a2) {
    var e2 = b2.height / b2.lineHeight | 0;
    e2 < 1 && (e2 = 1), e2 = b2.selection.positionLineNumber + e2 | 0;
    var c2 = u(b2.model);
    e2 > c2 && (e2 = c2), a2 ? s(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, e2, b2.selection.positionColumn) : p(b2, e2, b2.selection.positionColumn);
  }
  function r2(b2, a2) {
    var c2 = hostGetOffsetAt(b2.model.buffer, b2.selection.positionLineNumber, b2.selection.positionColumn, l);
    c2 > 0 && (c2 = c2 - 1 | 0), c2 = hostGetPositionAt(b2.model.buffer, c2, l);
    var d3 = x(n(b2.model, c2.lineNumber), c2.column);
    a2 ? s(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, c2.lineNumber, d3[1]) : p(b2, c2.lineNumber, d3[1]);
  }
  function w2(b2, a2) {
    var c2 = x(n(b2.model, b2.selection.positionLineNumber), b2.selection.positionColumn)[2];
    if (c2 == b2.selection.positionColumn) {
      var d3 = hostGetOffsetAt(b2.model.buffer, b2.selection.positionLineNumber, b2.selection.positionColumn, l);
      d3 < b2.model.buffer.length && (c2 = hostGetPositionAt(b2.model.buffer, d3 + 1 | 0, l), c2 = x(n(b2.model, c2.lineNumber), c2.column)[2]);
    }
    a2 ? s(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, b2.selection.positionLineNumber, c2) : p(b2, b2.selection.positionLineNumber, c2);
  }
  function y2(b2) {
    if (b2.readOnly) return;
    var c2, a2 = b2.selection;
    if (a2.startLineNumber != a2.endLineNumber || a2.startColumn != a2.endColumn) {
      K(b2, zf), P(b2, zf), m(b2);
      return;
    }
    c2 = x(n(b2.model, a2.positionLineNumber), a2.positionColumn)[1];
    c2 == a2.positionColumn && a2.positionColumn > 1 && (c2 = a2.positionColumn - 1 | 0), o(b2.model, [Q(a2.positionLineNumber, c2, a2.positionLineNumber, a2.positionColumn)], true), p(b2, a2.positionLineNumber, c2), m(b2);
  }
  function z2(b2) {
    if (b2.readOnly) return;
    var c2, d3, a2 = b2.selection;
    if (a2.startLineNumber != a2.endLineNumber || a2.startColumn != a2.endColumn) {
      K(b2, zf), P(b2, zf), m(b2);
      return;
    }
    c2 = x(n(b2.model, a2.positionLineNumber), a2.positionColumn)[2];
    c2 == a2.positionColumn && (c2 = a2.positionColumn + 1 | 0, d3 = t(b2.model, a2.positionLineNumber) + 1 | 0, c2 > d3 && (c2 = d3)), o(b2.model, [Q(a2.positionLineNumber, a2.positionColumn, a2.positionLineNumber, c2)], true), m(b2);
  }
  function A2(b2, a2) {
    if (b2.readOnly) return;
    var d3 = _i;
    b2.insertSpaces && (d3 = Ob(b2.tabSize));
    var e2, g2, c2, f2 = b2.selection;
    if (f2.startLineNumber != f2.endLineNumber || a2) {
      e2 = f2.startLineNumber;
      while (e2 <= f2.endLineNumber) {
        g2 = n(b2.model, e2);
        if (a2) {
          if (g2.length > 0 && g2.charAt(0) == _i) {
            c2 = 1;
          } else {
            c2 = 0;
            while (c2 < b2.tabSize && c2 < g2.length && g2.charAt(c2) == Hh) c2 = c2 + 1 | 0;
          }
          c2 > 0 && (g2 = b2.model, o(g2, [v(e2, 1, e2, c2 + 1 | 0, zf)], true));
        } else {
          o(b2.model, [v(e2, 1, e2, 1, d3)], true);
        }
        e2 = e2 + 1 | 0;
      }
      m(b2);
      return;
    }
    ca(b2, d3);
  }
  function B2(b2) {
    if (b2.readOnly) return;
    var c2 = n(b2.model, b2.selection.positionLineNumber), a2 = q(c2, 0, O(c2)), d3 = emptyBuf();
    b2.selection.positionColumn > 1 && (d3 = q(c2, b2.selection.positionColumn - 2 | 0, b2.selection.positionColumn - 1 | 0)), d3 == Nj && (a2 = a2 + "  "), a2 = xf + a2, K(b2, a2), P(b2, a2), m(b2);
  }
  function C2(b2, a2) {
    if (b2.readOnly) return;
    var d3 = b2.selection.startLineNumber, c2 = b2.selection.endLineNumber, e2 = d3 + a2 | 0, f2 = u(b2.model);
    if (e2 < 1 || (c2 + a2 | 0) > f2) return;
    f2 = [];
    while (d3 <= c2) f2.push(n(b2.model, d3)), e2 = e2 + 1 | 0;
    e2 = Ab(f2), a2 < 0 ? (a2 = d3 - 1 | 0, d3 = n(b2.model, a2), f2 = b2.model, o(f2, [v(a2, 1, c2, t(f2, c2) + 1 | 0, e2 + xf + d3)], true), c2 = c2 - 1 | 0, s(b2, a2, 1, c2, t(f2, c2) + 1 | 0)) : (a2 = c2 + 1 | 0, c2 = n(b2.model, a2), f2 = b2.model, o(f2, [v(d3, 1, a2, t(f2, a2) + 1 | 0, c2 + xf + e2)], true), c2 = d3 + 1 | 0, s(b2, c2, 1, a2, t(f2, a2) + 1 | 0)), m(b2);
  }
  Oc = function(l2, n2, o2) {
    if (n2 == Mf || "compositionType" == n2) {
      ca(l2, o2);
      return true;
    }
    if ("paste" == n2 || "editor.action.clipboardPasteAction" == n2) {
      Pa(l2, o2);
      return true;
    }
    if ("cut" == n2 || "editor.action.clipboardCutAction" == n2) {
      Oa(l2);
      return true;
    }
    if (n2 == Ai || "editor.action.clipboardCopyAction" == n2) {
      Na(l2);
      return true;
    }
    if (n2 == Hj || n2 == Hj) {
      a(l2);
      return true;
    }
    if ("deleteRight" == n2) {
      b(l2);
      return true;
    }
    if ("deleteWordLeft" == n2 || "deleteWordStartLeft" == n2) {
      y2(l2);
      return true;
    }
    if ("deleteWordRight" == n2 || "deleteWordEndRight" == n2) {
      z2(l2);
      return true;
    }
    if ("undo" == n2) {
      Ga(l2.model), Y(l2), m(l2);
      return true;
    }
    if ("redo" == n2) {
      Ha(l2.model), Y(l2), m(l2);
      return true;
    }
    if ("tab" == n2 || "editor.action.indentLines" == n2) {
      A2(l2, false);
      return true;
    }
    if ("outdent" == n2 || "editor.action.outdentLines" == n2) {
      A2(l2, true);
      return true;
    }
    if ("cursorLeft" == n2) {
      ta(l2, false);
      return true;
    }
    if ("cursorRight" == n2) {
      c(l2, false);
      return true;
    }
    if ("cursorUp" == n2) {
      d2(l2, false);
      return true;
    }
    if ("cursorDown" == n2) {
      e(l2, false);
      return true;
    }
    if ("cursorHome" == n2 || "cursorLineStart" == n2) {
      f(l2, false);
      return true;
    }
    if ("cursorEnd" == n2 || "cursorLineEnd" == n2) {
      g(l2, false);
      return true;
    }
    if ("cursorTop" == n2) {
      h(l2, false);
      return true;
    }
    if ("cursorBottom" == n2) {
      i2(l2, false);
      return true;
    }
    if ("cursorPageUp" == n2) {
      j2(l2, false);
      return true;
    }
    if ("cursorPageDown" == n2) {
      k2(l2, false);
      return true;
    }
    if ("cursorWordLeft" == n2 || "cursorWordStartLeft" == n2) {
      r2(l2, false);
      return true;
    }
    if ("cursorWordRight" == n2 || "cursorWordEndRight" == n2) {
      w2(l2, false);
      return true;
    }
    if ("cursorLeftSelect" == n2) {
      ta(l2, true);
      return true;
    }
    if ("cursorRightSelect" == n2) {
      c(l2, true);
      return true;
    }
    if ("cursorUpSelect" == n2) {
      d2(l2, true);
      return true;
    }
    if ("cursorDownSelect" == n2) {
      e(l2, true);
      return true;
    }
    if ("cursorHomeSelect" == n2) {
      f(l2, true);
      return true;
    }
    if ("cursorEndSelect" == n2) {
      g(l2, true);
      return true;
    }
    if ("cursorWordLeftSelect" == n2) {
      r2(l2, true);
      return true;
    }
    if ("cursorWordRightSelect" == n2) {
      w2(l2, true);
      return true;
    }
    if ("selectAll" == n2) {
      n2 = u(l2.model), s(l2, 1, 1, n2, t(l2.model, n2) + 1 | 0);
      return true;
    }
    if (n2 == Sh) {
      Hc(l2);
      return true;
    }
    if ("enter" == n2) {
      B2(l2);
      return true;
    }
    if ("editor.action.deleteLines" == n2) {
      Ic(l2);
      return true;
    }
    if ("editor.action.moveLinesUpAction" == n2) {
      C2(l2, -1);
      return true;
    }
    if ("editor.action.moveLinesDownAction" == n2) {
      C2(l2, 1);
      return true;
    }
    if ("editor.action.copyLinesUpAction" == n2) {
      Bb(l2, -1);
      return true;
    }
    if ("editor.action.copyLinesDownAction" == n2) {
      Bb(l2, 1);
      return true;
    }
    if ("editor.action.insertCursorAbove" == n2) {
      Cb(l2, -1);
      return true;
    }
    if ("editor.action.insertCursorBelow" == n2) {
      Cb(l2, 1);
      return true;
    }
    if ("editor.action.trimTrailingWhitespace" == n2) {
      Jc(l2);
      return true;
    }
    if ("editor.action.insertLineAfter" == n2) {
      Kc(l2);
      return true;
    }
    if ("editor.action.insertLineBefore" == n2) {
      Lc(l2);
      return true;
    }
    if ("editor.action.joinLines" == n2) {
      Mc(l2);
      return true;
    }
    if (n2 == gi) {
      Nc(l2);
      return true;
    }
    if ("editor.action.fontZoomIn" == n2) {
      l2.fontSize = l2.fontSize + 1 | 0, l2.lineHeight = l2.fontSize + 5 | 0, m(l2);
      return true;
    }
    if ("editor.action.fontZoomOut" == n2) {
      l2.fontSize > 8 && (l2.fontSize = l2.fontSize - 1 | 0, l2.lineHeight = l2.fontSize + 5 | 0, m(l2));
      return true;
    }
    if ("editor.action.fontZoomReset" == n2) {
      l2.fontSize = 14, l2.lineHeight = 19, m(l2);
      return true;
    }
    return false;
  };
  Pc = function(l2, n2) {
    var o2 = eventKey(n2), q2 = eventCtrlKey(n2), p2 = eventShiftKey(n2), H2 = eventAltKey(n2);
    if (q2 && !p2 && ("z" == o2 || "Z" == o2)) {
      preventDefault(n2), Ga(l2.model), Y(l2), m(l2);
      return;
    }
    if (q2 && ("y" == o2 || "Y" == o2 || p2 && ("z" == o2 || "Z" == o2))) {
      preventDefault(n2), Ha(l2.model), Y(l2), m(l2);
      return;
    }
    if (q2 && ("a" == o2 || "A" == o2)) {
      preventDefault(n2), n2 = u(l2.model), s(l2, 1, 1, n2, t(l2.model, n2) + 1 | 0);
      return;
    }
    if (q2 && ("c" == o2 || "C" == o2)) {
      preventDefault(n2), Na(l2);
      return;
    }
    if (q2 && ("x" == o2 || "X" == o2)) {
      preventDefault(n2), Oa(l2);
      return;
    }
    if (q2 && ("v" == o2 || "V" == o2)) {
      preventDefault(n2), Pa(l2, zf);
      return;
    }
    if ("Backspace" == o2) {
      preventDefault(n2), q2 ? y2(l2) : a(l2);
      return;
    }
    if ("Delete" == o2) {
      preventDefault(n2), q2 ? z2(l2) : b(l2);
      return;
    }
    if (o2 == hh) {
      preventDefault(n2), B2(l2);
      return;
    }
    if ("Tab" == o2) {
      preventDefault(n2), A2(l2, p2);
      return;
    }
    if ("Home" == o2) {
      preventDefault(n2), q2 ? h(l2, p2) : f(l2, p2);
      return;
    }
    if ("End" == o2) {
      preventDefault(n2), q2 ? i2(l2, p2) : g(l2, p2);
      return;
    }
    if ("PageUp" == o2) {
      preventDefault(n2), j2(l2, p2);
      return;
    }
    if ("PageDown" == o2) {
      preventDefault(n2), k2(l2, p2);
      return;
    }
    if ("ArrowLeft" == o2) {
      preventDefault(n2), q2 ? r2(l2, p2) : ta(l2, p2);
      return;
    }
    if ("ArrowRight" == o2) {
      preventDefault(n2), q2 ? w2(l2, p2) : c(l2, p2);
      return;
    }
    if ("ArrowUp" == o2) {
      preventDefault(n2), H2 ? C2(l2, -1) : d2(l2, p2);
      return;
    }
    if (o2 == ck) {
      preventDefault(n2), H2 ? C2(l2, 1) : e(l2, p2);
      return;
    }
  };
})();
function Qc(b, a) {
  preventDefault(a), Pa(b, clipboardReadEvent(a));
}
function Db(g, a) {
  preventDefault(a), g = V(g), clipboardWriteEvent(a, g), clipboardWrite(g);
}
function Rc(b, a) {
  Db(b, a), Oa(b);
}
function Sc(a, b, c, d2) {
  a[0] = b, a[1] = c, a[2] = d2;
}
function Eb(a) {
  if (!a) return Lj;
  var b = a + "";
  return b.length > 0 && "[object Object]" != b || a.language ? a.language + "" : a[0] ? Eb(a[0]) : Lj;
}
function y(a, b, c) {
  b = ["", "", null], Sc(b, a, Eb(b), c), L.push(b);
  return function() {
    var c2 = 0;
    while (c2 < L.length) {
      if (L[c2] == b) {
        L.splice(c2, 1);
        return;
      }
      c2++;
    }
  };
}
function Tc(a, b) {
  return a == Lj || a == b || 0 == a.length;
}
function Fb(h) {
  if (h) {
    if (h.range) return Fb(h.range);
    var a = h.startLineNumber ? +h.startLineNumber | 0 : 1, b = h.startColumn ? +h.startColumn | 0 : 1, c = h.endLineNumber ? +h.endLineNumber | 0 : 1;
    h = h.endColumn ? +h.endColumn | 0 : 1;
  } else {
    a = 1, b = 1, c = 1, h = 1;
  }
  var d2 = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
  z(d2, a, b, c, h);
  return d2;
}
function ua(b) {
  b.findOpen = true, setDisplay(b.findWidget, mf), V(b).length > 0 && (b.findQuery = V(b), inputSetValue(b.findInput, b.findQuery)), focusElement(b.findInput);
}
function Qa(b) {
  b.findQuery = inputGetValue(b.findInput);
  return 0 == b.findQuery.length ? [] : E(b.model.buffer, b.findQuery, false, true, false, Kk);
}
function va(b, a) {
  var c = Qa(b);
  if (0 == c.length) return;
  a ? (b.findIndex = b.findIndex - 1 | 0, b.findIndex < 0 && (b.findIndex = c.length - 1)) : (b.findIndex = b.findIndex + 1 | 0, b.findIndex >= c.length && (b.findIndex = 0)), a = c[b.findIndex], s(b, a.range.startLineNumber, a.range.startColumn, a.range.endLineNumber, a.range.endColumn), m(b);
}
var Gb;
var Ra;
var Wc;
var Xc;
var Yc;
var Zc;
(function() {
  function a(a2, b2) {
    var d3 = [], c2 = 0;
    while (c2 < L.length) L[c2][0] == a2 && Tc(L[c2][1], b2) && d3.push(L[c2][2]), c2++;
    return d3;
  }
  function b(a2, b2, c2, d3, e) {
    return hostCall(a2, b2, c2, d3, e);
  }
  function c(a2) {
    var e, b2, h, d3, f = [], g = jsArrayLen(a2), c2 = 0;
    while (c2 < g) e = jsArrayAt(a2, c2), b2 = jsPropString(e, $h, zf), h = jsPropString(e, Ij, b2), 0 == b2.length && (b2 = jsPropString(e, Ij, zf)), b2.length > 0 && (d3 = { label: "", insertText: "", kind: 0, detail: "" }, d3.label = b2, d3.insertText = h, d3.kind = 18, d3.detail = emptyBuf(), d3.kind = jsPropInt(e, "kind", 18), f.push(d3)), c2++;
    return f;
  }
  function d2(a2) {
    if (!a2) return emptyBuf();
    var c2 = jsPropString(a2, nh, zf);
    if (c2.length > 0) return c2;
    var b2 = a2.contents, d3 = jsArrayLen(b2);
    if (0 == d3) {
      return jsArrayLen(a2) > 0 ? jsPropString(jsArrayAt(a2, 0), nh, jsPropString(jsArrayAt(a2, 0), $h, zf)) : a2 + "";
    }
    a2 = emptyBuf();
    c2 = 0;
    while (c2 < d3) {
      var e = jsArrayAt(b2, c2);
      e = jsPropString(e, nh, e + ""), a2.length > 0 && (a2 = a2 + xf), a2 += e, c2++;
    }
    return a2;
  }
  Gb = function(d3) {
    var f = d3.model, g = d3.selection.positionColumn, e = { lineNumber: 0, column: 0 };
    e.lineNumber = d3.selection.positionLineNumber, e.column = g, f = qb(f, e);
    var h = a(Ej, d3.languageId);
    e = 0;
    while (e < h.length) {
      var i2 = c(b(h[e], "provideCompletionItems", d3.modelFacade, { __proto__: null, lineNumber: d3.selection.positionLineNumber, column: d3.selection.positionColumn }, void 0));
      g = 0;
      while (g < i2.length) f.push(i2[g]), g++;
      e++;
    }
    d3.suggestItems = f;
    d3.suggestIndex = 0, d3.suggestOpen = f.length > 0;
    if (!d3.suggestOpen) {
      setDisplay(d3.suggestWidget, lf);
      return;
    }
    g = emptyBuf();
    e = 0;
    while (e < f.length && e < 12) h = zf, e == d3.suggestIndex && (h = " background:#04395e;"), g = g + '<div data-suggest="' + e.toString(10) + '" style="padding:2px 6px;' + h + dj + d3.suggestItems[e].label + Kg, e++;
    setInnerHTML(d3.suggestWidget, g), f = ((d3.selection.positionColumn - 1 | 0) * d3.charWidth | 0) + 56 | 0, setStyle(d3.suggestWidget, Kh, (d3.selection.positionLineNumber * d3.lineHeight | 0).toString(10) + bh), setStyle(d3.suggestWidget, ah, f.toString(10) + bh), setDisplay(d3.suggestWidget, mf);
  };
  Ra = function(c2) {
    var g = c2.selection.positionColumn, e = { lineNumber: 0, column: 0 };
    e.lineNumber = c2.selection.positionLineNumber, e.column = g, g = x(n(c2.model, e.lineNumber), e.column)[0];
    var i2, h = a("hover", c2.languageId), f = 0;
    while (f < h.length) i2 = d2(b(h[f], "provideHover", c2.modelFacade, { __proto__: null, lineNumber: e.lineNumber, column: e.column }, void 0)), i2.length > 0 && (g.length > 0 && (g = g + xf), g = g + i2), f++;
    h = ma(zf, X(c2.model.uri), 0), f = 0;
    while (f < h.length) h[f].startLineNumber == e.lineNumber && (g.length > 0 && (g = g + xf), g = g + h[f].message), f++;
    c2.hoverOpen = g.length > 0;
    if (!c2.hoverOpen) {
      setDisplay(c2.hoverWidget, lf);
      return;
    }
    setTextContent(c2.hoverWidget, g);
    setStyle(c2.hoverWidget, Kh, (e.lineNumber * c2.lineHeight | 0).toString(10) + bh), setStyle(c2.hoverWidget, ah, (((e.column - 1 | 0) * c2.charWidth | 0) + 56 | 0).toString(10) + bh), setDisplay(c2.hoverWidget, mf);
  }, Wc = function(c2) {
    var g = c2.selection.positionColumn;
    g = x(n(c2.model, c2.selection.positionLineNumber), g)[0] + "(value, options)";
    var e = a(Vi, c2.languageId);
    e.length > 0 && (e = d2(b(e[0], "provideSignatureHelp", c2.modelFacade, { __proto__: null, lineNumber: c2.selection.positionLineNumber, column: c2.selection.positionColumn }, void 0)), e.length > 0 && (g = e)), setTextContent(c2.paramWidget, g), setDisplay(c2.paramWidget, mf);
  }, Xc = function(c2) {
    var d3 = a(Gj, c2.languageId);
    if (d3.length > 0) {
      if (d3 = b(d3[0], "provideDefinition", c2.modelFacade, { __proto__: null, lineNumber: c2.selection.positionLineNumber, column: c2.selection.positionColumn }, void 0)) {
        !d3[0] || (d3 = d3[0]), d3 = Fb(d3), p(c2, d3.startLineNumber, d3.startColumn), m(c2);
        return;
      }
    }
    d3 = E(c2.model.buffer, x(n(c2.model, c2.selection.positionLineNumber), c2.selection.positionColumn)[0], false, true, true, Kk);
    d3.length > 0 && (p(c2, d3[0].range.startLineNumber, d3[0].range.startColumn), m(c2));
  }, Yc = function(c2) {
    var d3 = a(Uh, c2.languageId);
    if (d3.length > 0) {
      d3 = b(d3[0], "provideDocumentFormattingEdits", c2.modelFacade, void 0, void 0);
      var f, h, g = d3 ? 1 : 0;
      if (g > 0 && d3[0]) {
        g = emptyBuf(), !d3[0].text || (d3 = d3[0], g = d3.text + "");
        if (g.length > 0) {
          d3 = u(c2.model), o(c2.model, [v(1, 1, d3, t(c2.model, d3) + 1 | 0, g)], true), m(c2);
          return;
        }
      }
    }
    d3 = c2.model;
    g = d3.buffer, g = nc(I(g, g.root)), f = g.replace(/\r\n|\r|\n/g, xf), h = H(f), g = { buffer: "", lineStarts: [] }, g.buffer = f, g.lineStarts = h, g = [g], f = { root: null, buffers: [], lineCnt: 0, length: 0, eol: "", eolLength: 0, eolNormalized: false, lastChangeBufferPos: null, cacheNode: null, cacheNodeStartOffset: 0, cacheNodeStartLineNumber: 0, cacheHasLine: false, cacheValid: false, lastVisitedLineNumber: 0, lastVisitedLineValue: "", posNode: null, posRemainder: 0, posStart: 0, walkLine: 0, walkCol: 0, tmpBuffer: null }, fa(f, g, xf, true), d3.buffer = f, d3.versionId = d3.versionId + 1 | 0, B(d3.onDidChangeContent), m(c2);
  }, Zc = function(d3) {
    var e, g, c2, i2 = a(Dj, d3.languageId), f = emptyBuf(), h = 0;
    while (h < i2.length) {
      e = b(i2[h], "provideCodeActions", d3.modelFacade, { __proto__: null, startLineNumber: d3.selection.startLineNumber, startColumn: d3.selection.startColumn, endLineNumber: d3.selection.endLineNumber, endColumn: d3.selection.endColumn }, void 0), g = jsArrayLen(e), e && e.actions && (g = jsArrayLen(e.actions), e = e.actions), c2 = 0;
      while (c2 < g) f += "<div>", f = f + jsPropString(jsArrayAt(e, c2), "title", jsPropString(jsArrayAt(e, c2), $h, "fix")) + Kg, c2++;
      h++;
    }
    0 == f.length && (f = "<div>No code actions</div>");
    setInnerHTML(d3.hoverWidget, f), setDisplay(d3.hoverWidget, mf), d3.hoverOpen = true;
  };
})();
function Hb(b) {
  var a, c, e, f, g;
  if (!b.suggestOpen || 0 == b.suggestItems.length) return;
  c = b.suggestItems[b.suggestIndex], a = x(n(b.model, b.selection.positionLineNumber), b.selection.positionColumn), e = b.selection.positionLineNumber, f = a[1], g = b.selection.positionLineNumber, o(b.model, [v(e, f, g, a[2], rb(c.insertText))], true), b.suggestOpen = false, setDisplay(b.suggestWidget, lf), m(b);
}
function Ib(b, a) {
  if (!b.suggestOpen) return;
  b.suggestIndex = b.suggestIndex + a | 0, b.suggestIndex < 0 && (b.suggestIndex = b.suggestItems.length - 1), b.suggestIndex >= b.suggestItems.length && (b.suggestIndex = 0), Gb(b);
}
function Uc(b) {
  var e = inputGetValue(b.gotoInput);
  e = e.length > 0 ? +e | 0 : 1;
  var a = u(b.model);
  e < 1 && (e = 1), e > a || (a = e), p(b, a, 1), b.gotoOpen = false, setDisplay(b.gotoWidget, lf), m(b), focusElement(b.textarea);
}
function Vc(b) {
  var d2 = inputGetValue(b.renameInput);
  if (b.renameWord.length > 0 && d2.length > 0) {
    var e = E(b.model.buffer, b.renameWord, false, true, true, Kk), c = e.length - 1;
    while (c >= 0) {
      var a = e[c], f = b.model;
      o(f, [v(a.range.startLineNumber, a.range.startColumn, a.range.endLineNumber, a.range.endColumn, d2)], true), c--;
    }
  }
  b.renameOpen = false;
  setDisplay(b.renameWidget, lf), m(b), focusElement(b.textarea);
}
function Jb(b, a, c) {
  b.contextOpen = true, setInnerHTML(b.contextWidget, '<div data-cmd="editor.action.clipboardCutAction">Cut</div><div data-cmd="editor.action.clipboardCopyAction">Copy</div><div data-cmd="editor.action.clipboardPasteAction">Paste</div><div data-cmd="editor.action.commentLine">Toggle Line Comment</div><div data-cmd="editor.action.formatDocument">Format Document</div><div data-cmd="editor.action.rename">Rename Symbol</div><div data-cmd="editor.action.goToDefinition">Go to Definition</div><div data-cmd="editor.action.peekDefinition">Peek References</div>'), setStyle(b.contextWidget, Kh, c.toString(10) + bh), setStyle(b.contextWidget, ah, a.toString(10) + bh), setDisplay(b.contextWidget, mf);
}
function _c(a, b, c, d2) {
  a.id = b, a.label = c, a.run = d2;
}
function W(a, b, c) {
  if (!a) return c;
  a = a[b];
  return !a ? c : a + "";
}
function $c(a, b) {
  a.view = b;
  let c = { listeners: [], disposed: false };
  c.listeners = [], c.disposed = false, a.contentEmitter = c, c = { listeners: [], disposed: false }, c.listeners = [], c.disposed = false, a.cursorEmitter = c, a.disposed = false, a.actions = [], a.modelFacade = void 0, b.model.onDidChangeContent.listeners.push(function() {
    B(a.contentEmitter);
  }), ad(a);
}
function ad(a) {
  let b = a.view, d2 = "keydown", c = false;
  b.textarea.addEventListener(d2, function(c2) {
    if (b.suggestOpen) {
      var d3 = eventKey(c2);
      if (d3 == ck) {
        preventDefault(c2), Ib(b, 1);
        return;
      }
      if ("ArrowUp" == d3) {
        preventDefault(c2), Ib(b, -1);
        return;
      }
      if (d3 == hh || "Tab" == d3) {
        preventDefault(c2), Hb(b);
        return;
      }
      if (d3 == Ji) {
        preventDefault(c2), b.suggestOpen = false, setDisplay(b.suggestWidget, lf);
        return;
      }
    }
    if (eventCtrlKey(c2) && "f" == eventKey(c2)) {
      preventDefault(c2), ua(b);
      return;
    }
    if (eventCtrlKey(c2) && "h" == eventKey(c2)) {
      preventDefault(c2), ua(b);
      return;
    }
    if (eventCtrlKey(c2) && "g" == eventKey(c2)) {
      preventDefault(c2), C(a, uh, void 0);
      return;
    }
    if (eventCtrlKey(c2) && eventKey(c2) == Hh) {
      preventDefault(c2), C(a, Hg, void 0);
      return;
    }
    if (eventCtrlKey(c2) && "/" == eventKey(c2)) {
      preventDefault(c2), C(a, Yg, void 0);
      return;
    }
    if (eventCtrlKey(c2) && ("d" == eventKey(c2) || "D" == eventKey(c2))) {
      preventDefault(c2), C(a, Qf, void 0);
      return;
    }
    if ("F2" == eventKey(c2)) {
      preventDefault(c2), C(a, Gh, void 0);
      return;
    }
    if ("F12" == eventKey(c2)) {
      preventDefault(c2), eventShiftKey(c2) ? C(a, Gg, void 0) : C(a, Fg, void 0);
      return;
    }
    if ("F8" == eventKey(c2)) {
      preventDefault(c2), C(a, Zg, void 0);
      return;
    }
    eventKey(c2) == Ji && (b.suggestOpen = false, d3 = lf, setDisplay(b.suggestWidget, lf), b.hoverOpen = false, setDisplay(b.hoverWidget, lf), b.contextOpen = false, setDisplay(b.contextWidget, lf), b.findOpen = false, setDisplay(b.findWidget, lf), focusElement(b.textarea));
    Pc(b, c2), B(a.cursorEmitter);
  }, c), b.textarea.addEventListener("compositionend", function(a2) {
    a2 = a2.data + "", a2.length > 0 && ca(b, a2);
  }, c), b.textarea.addEventListener(_h, function(h) {
    h = h.target, h = h.value + "", h.length > 0 && ca(b, h);
  }, c), b.textarea.addEventListener("paste", function(a2) {
    Qc(b, a2);
  }, c), b.textarea.addEventListener(Ai, function(a2) {
    Db(b, a2);
  }, c), b.textarea.addEventListener("cut", function(a2) {
    Rc(b, a2);
  }, c), b.scrollable.addEventListener("scroll", function(a2) {
    b.scrollTop = +b.scrollable.scrollTop | 0, m(b);
  }, c);
  let e = "mousedown";
  b.scrollable.addEventListener(e, function(c2) {
    b.contextOpen = false;
    var d3 = lf;
    setDisplay(b.contextWidget, lf), b.hoverOpen = false, setDisplay(b.hoverWidget, lf);
    var e2 = ra(b, eventClientX(c2), eventClientY(c2));
    d3 = e2.lineNumber, e2 = e2.column;
    var f = eventDetail(c2);
    b.mouseSelecting = true, b.mouseAnchorLine = d3, b.mouseAnchorColumn = e2, f >= 3 ? s(b, d3, 1, d3, t(b.model, d3) + 1 | 0) : 2 == f ? (C(a, gi, void 0), p(b, d3, e2), C(a, Sh, void 0)) : eventShiftKey(c2) ? s(b, b.selection.selectionStartLineNumber, b.selection.selectionStartColumn, d3, e2) : p(b, d3, e2), focusElement(b.textarea), m(b), B(a.cursorEmitter);
  }, c), b.margin.addEventListener(e, function(a2) {
    qa(b, ra(b, eventClientX(a2), eventClientY(a2)).lineNumber);
  }, c), b.root.addEventListener("contextmenu", function(a2) {
    preventDefault(a2), Jb(b, eventClientX(a2), eventClientY(a2));
  }, c), b.contextWidget.addEventListener(e, function(c2) {
    c2 = hostCall(c2.target, "getAttribute", "data-cmd", void 0, void 0) + "", c2.length > 0 && c2 != Rh && C(a, c2, void 0), b.contextOpen = false, setDisplay(b.contextWidget, lf);
  }, c), b.findInput.addEventListener(d2, function(a2) {
    eventKey(a2) == hh && (preventDefault(a2), va(b, eventCtrlKey(a2))), eventKey(a2) == Ji && (preventDefault(a2), b.findOpen = false, setDisplay(b.findWidget, lf), focusElement(b.textarea));
  }, c), b.gotoInput.addEventListener(d2, function(a2) {
    eventKey(a2) == hh && (preventDefault(a2), Uc(b));
  }, c), b.renameInput.addEventListener(d2, function(a2) {
    eventKey(a2) == hh && (preventDefault(a2), Vc(b));
  }, c), b.scrollable.addEventListener("mousemove", function(c2) {
    if (b.mouseSelecting) {
      c2 = ra(b, eventClientX(c2), eventClientY(c2)), s(b, b.mouseAnchorLine, b.mouseAnchorColumn, c2.lineNumber, c2.column), m(b), B(a.cursorEmitter);
      return;
    }
    if (eventCtrlKey(c2)) {
      var D2 = ra(b, eventClientX(c2), eventClientY(c2));
      p(b, D2.lineNumber, D2.column), Ra(b);
    }
  }, c), b.scrollable.addEventListener("mouseup", function(a2) {
    b.mouseSelecting = false;
  }, c), b.root.addEventListener("mouseleave", function(a2) {
    b.mouseSelecting = false;
  }, c);
}
function Sa(a) {
  var b = +a.view.root.clientWidth | 0, c = +a.view.root.clientHeight | 0;
  b < 1 && (b = a.view.width), c < 1 && (c = a.view.height), tb(a.view, b, c);
}
var C = /* @__PURE__ */ (function() {
  function a(f2, a2, b2) {
    b2 = J(b2);
    var c2 = jg, d3 = ig;
    b2 && b2.blockCommentStart.length > 0 && (c2 = b2.blockCommentStart, d3 = b2.blockCommentEnd), b2 = Aa(f2.buffer, a2), 0 == b2.indexOf(c2) && b2.length >= (c2.length + d3.length | 0) ? o(f2, [v(a2.startLineNumber, a2.startColumn, a2.endLineNumber, a2.endColumn, q(b2, c2.length, b2.length - d3.length))], true) : o(f2, [v(a2.startLineNumber, a2.startColumn, a2.endLineNumber, a2.endColumn, c2 + b2 + d3)], true);
  }
  function b(f2, a2) {
    a2 = x(n(f2, a2.lineNumber), a2.column)[0];
    if (0 == a2.length) return;
    E(f2.buffer, a2, false, true, true, 200);
  }
  function c(f2) {
    var b2, a2, c2, d3, h2 = u(f2), e2 = 1;
    while (e2 <= h2) {
      b2 = n(f2, e2), a2 = b2.indexOf("http://"), a2 < 0 && (a2 = b2.indexOf("https://"));
      if (a2 >= 0) {
        d3 = 0;
        while ((a2 + d3 | 0) < b2.length) {
          c2 = b2.charAt(a2 + d3 | 0);
          if (c2 == Hh || c2 == _i || ")" == c2 || '"' == c2) break;
          d3 = d3 + 1 | 0;
        }
        z({ startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 }, e2, a2 + 1 | 0, e2, (a2 + d3 | 0) + 1 | 0);
      }
      e2 = e2 + 1 | 0;
    }
  }
  function d2(f2, a2) {
    var c2, d3, g2 = [], b2 = O(n(f2, a2)), e2 = 1;
    while (e2 < a2) c2 = n(f2, e2), d3 = O(c2), d3 < b2 && d3 != c2.length && (g2.push(c2), b2 = d3), e2 = e2 + 1 | 0;
    return g2;
  }
  function e(g2, a2) {
    if (a2) return f(g2);
    a2 = emptyBuf();
    var b2, c2 = 0;
    while (c2 < g2.length) b2 = g2.charCodeAt(c2), a2 = b2 >= 65 && b2 <= 90 ? a2 + String.fromCharCode(b2 + 32) : a2 + g2.charAt(c2), c2++;
    return a2;
  }
  function f(g2) {
    var b2, a2 = emptyBuf(), c2 = 0;
    while (c2 < g2.length) b2 = g2.charCodeAt(c2), a2 = b2 >= 97 && b2 <= 122 ? a2 + String.fromCharCode(b2 - 32) : a2 + g2.charAt(c2), c2++;
    return a2;
  }
  function g(b2) {
    b2.replaceQuery = inputGetValue(b2.replaceInput);
    var a2 = Qa(b2);
    if (0 == a2.length) return;
    b2.findIndex >= a2.length && (b2.findIndex = 0), a2 = a2[b2.findIndex], o(b2.model, [v(a2.range.startLineNumber, a2.range.startColumn, a2.range.endLineNumber, a2.range.endColumn, b2.replaceQuery)], true), m(b2), va(b2, false);
  }
  function h(b2) {
    b2.replaceQuery = inputGetValue(b2.replaceInput);
    var d3 = Qa(b2), c2 = d3.length - 1;
    while (c2 >= 0) {
      var a2 = d3[c2], e2 = b2.model;
      o(e2, [v(a2.range.startLineNumber, a2.range.startColumn, a2.range.endLineNumber, a2.range.endColumn, b2.replaceQuery)], true), c2--;
    }
    m(b2);
  }
  function i2(b2) {
    let a2 = x(n(b2.model, b2.selection.positionLineNumber), b2.selection.positionColumn);
    b2.renameWord = a2[0], b2.renameOpen = true, inputSetValue(b2.renameInput, a2[0]), setDisplay(b2.renameWidget, mf), focusElement(b2.renameInput);
  }
  function j2(b2) {
    var d3 = E(b2.model.buffer, x(n(b2.model, b2.selection.positionLineNumber), b2.selection.positionColumn)[0], false, true, true, Kk), a2 = emptyBuf(), c2 = 0;
    while (c2 < d3.length && c2 < 20) a2 = a2 + '<div data-line="' + d3[c2].range.startLineNumber.toString(10) + dj + d3[c2].range.startLineNumber.toString(10) + ": " + (d3[c2].matches[0] || "") + Kg, c2++;
    setInnerHTML(b2.hoverWidget, a2), setDisplay(b2.hoverWidget, mf), b2.hoverOpen = true;
  }
  function k2(b2) {
    var c2 = x(n(b2.model, b2.selection.positionLineNumber), b2.selection.positionColumn), a2 = V(b2);
    0 == a2.length && (a2 = c2[0], s(b2, b2.selection.positionLineNumber, c2[1], b2.selection.positionLineNumber, c2[2]));
    var e2 = E(b2.model.buffer, a2, false, true, true, Kk);
    c2 = 0;
    while (c2 < e2.length) {
      a2 = e2[c2];
      var d3 = a2.range.startLineNumber == b2.selection.startLineNumber && a2.range.startColumn == b2.selection.startColumn;
      if (!d3) {
        b2.extraCursors.push(b2.selection), s(b2, a2.range.startLineNumber, a2.range.startColumn, a2.range.endLineNumber, a2.range.endColumn), m(b2);
        return;
      }
      c2++;
    }
  }
  function l2(f2, l3, q2) {
    if ("actions.find" == l3 || "editor.action.startFindAction" == l3) {
      ua(f2);
      return true;
    }
    if ("editor.action.startFindReplaceAction" == l3) {
      ua(f2);
      return true;
    }
    if ("editor.action.nextMatchFindAction" == l3) {
      va(f2, false);
      return true;
    }
    if ("editor.action.previousMatchFindAction" == l3) {
      va(f2, true);
      return true;
    }
    if ("editor.action.replaceOne" == l3) {
      g(f2);
      return true;
    }
    if ("editor.action.replaceAll" == l3) {
      h(f2);
      return true;
    }
    if ("closeFindWidget" == l3) {
      f2.findOpen = false, setDisplay(f2.findWidget, lf), focusElement(f2.textarea);
      return true;
    }
    if (l3 == Hg) {
      Gb(f2);
      return true;
    }
    if ("acceptSelectedSuggestion" == l3) {
      Hb(f2);
      return true;
    }
    if ("hideSuggestWidget" == l3) {
      f2.suggestOpen = false, setDisplay(f2.suggestWidget, lf);
      return true;
    }
    if ("editor.action.showHover" == l3) {
      Ra(f2);
      return true;
    }
    if (l3 == uh) {
      f2.gotoOpen = true, setDisplay(f2.gotoWidget, mf), focusElement(f2.gotoInput);
      return true;
    }
    if (l3 == Gh) {
      i2(f2);
      return true;
    }
    if ("editor.action.triggerParameterHints" == l3) {
      Wc(f2);
      return true;
    }
    if ("closeParameterHints" == l3) {
      setDisplay(f2.paramWidget, lf);
      return true;
    }
    if (l3 == Yg) {
      sb(f2.model, f2.selection.positionLineNumber, f2.languageId), m(f2);
      return true;
    }
    if ("editor.action.blockComment" == l3) {
      q2 = f2.selection.startLineNumber;
      var t2 = f2.selection.startColumn, u2 = f2.selection.endLineNumber, w2 = f2.selection.endColumn, y2 = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
      z(y2, q2, t2, u2, w2), a(f2.model, y2, f2.languageId), m(f2);
      return true;
    }
    if ("editor.action.jumpToBracket" == l3) {
      t2 = f2.selection.positionColumn, l3 = { lineNumber: 0, column: 0 }, l3.lineNumber = f2.selection.positionLineNumber, l3.column = t2, l3 = la(f2.model, l3), l3 && (p(f2, l3.endLineNumber, l3.endColumn), m(f2));
      return true;
    }
    if ("editor.action.selectToBracket" == l3) {
      t2 = f2.selection.positionColumn, l3 = { lineNumber: 0, column: 0 }, l3.lineNumber = f2.selection.positionLineNumber, l3.column = t2, l3 = la(f2.model, l3), l3 && (s(f2, l3.startLineNumber, l3.startColumn, l3.endLineNumber, l3.endColumn), m(f2));
      return true;
    }
    if ("editor.fold" == l3) {
      qa(f2, f2.selection.positionLineNumber);
      return true;
    }
    if ("editor.unfold" == l3) {
      qa(f2, f2.selection.positionLineNumber);
      return true;
    }
    if ("editor.foldAll" == l3) {
      wb(f2, true);
      return true;
    }
    if ("editor.unfoldAll" == l3) {
      wb(f2, false);
      return true;
    }
    if ("editor.action.formatDocument" == l3) {
      Yc(f2);
      return true;
    }
    if (l3 == Fg || "editor.action.revealDefinition" == l3 || "editor.action.goToDeclaration" == l3) {
      Xc(f2);
      return true;
    }
    if (l3 == Gg || "editor.action.referenceSearch.trigger" == l3 || "editor.action.peekDefinition" == l3) {
      j2(f2);
      return true;
    }
    if (l3 == Qf) {
      k2(f2);
      return true;
    }
    if ("editor.action.selectHighlights" == l3 || "editor.action.changeAll" == l3) {
      l3 = E(f2.model.buffer, x(n(f2.model, f2.selection.positionLineNumber), f2.selection.positionColumn)[0], false, true, true, Kk), l3.length > 0 && (s(f2, l3[0].range.startLineNumber, l3[0].range.startColumn, l3[l3.length - 1].range.endLineNumber, l3[l3.length - 1].range.endColumn), m(f2));
      return true;
    }
    if ("editor.action.wordHighlight.trigger" == l3) {
      t2 = f2.selection.positionColumn, l3 = { lineNumber: 0, column: 0 }, l3.lineNumber = f2.selection.positionLineNumber, l3.column = t2, b(f2.model, l3);
      return true;
    }
    if ("editor.action.openLink" == l3) {
      c(f2.model);
      return true;
    }
    if ("editor.action.transformToUppercase" == l3) {
      l3 = V(f2), l3.length > 0 && (q2 = f2.model, t2 = f2.selection.startLineNumber, u2 = f2.selection.startColumn, w2 = f2.selection.endLineNumber, y2 = f2.selection.endColumn, o(q2, [v(t2, u2, w2, y2, e(l3, true))], true), m(f2));
      return true;
    }
    if ("editor.action.transformToLowercase" == l3) {
      l3 = V(f2), l3.length > 0 && (q2 = f2.model, t2 = f2.selection.startLineNumber, u2 = f2.selection.startColumn, w2 = f2.selection.endLineNumber, y2 = f2.selection.endColumn, o(q2, [v(t2, u2, w2, y2, e(l3, false))], true), m(f2));
      return true;
    }
    if ("editor.action.insertSnippet" == l3) {
      ca(f2, rb(q2));
      return true;
    }
    if ("editor.action.quickOutline" == l3 || "editor.action.gotoSymbol" == l3) {
      j2(f2);
      return true;
    }
    if (l3 == Zg || "editor.action.gotoErrorNext" == l3 || "editor.action.marker.nextInFiles" == l3) {
      r2(f2, false);
      return true;
    }
    if ("editor.action.marker.prev" == l3 || "editor.action.gotoErrorPrev" == l3) {
      r2(f2, true);
      return true;
    }
    if ("editor.action.quickFix" == l3 || "editor.action.codeAction" == l3) {
      Zc(f2);
      return true;
    }
    if ("editor.action.smartSelect.expand" == l3) {
      t2 = f2.selection.positionColumn, l3 = { lineNumber: 0, column: 0 }, l3.lineNumber = f2.selection.positionLineNumber, l3.column = t2, l3 = la(f2.model, l3), l3 ? (s(f2, l3.startLineNumber, l3.startColumn, l3.endLineNumber, l3.endColumn), m(f2)) : (l3 = x(n(f2.model, f2.selection.positionLineNumber), f2.selection.positionColumn), s(f2, f2.selection.positionLineNumber, l3[1], f2.selection.positionLineNumber, l3[2]), m(f2));
      return true;
    }
    if ("editor.action.smartSelect.shrink" == l3) {
      p(f2, f2.selection.positionLineNumber, f2.selection.positionColumn), m(f2);
      return true;
    }
    if ("editor.toggleFold" == l3 || "editor.foldRecursively" == l3 || "editor.unfoldRecursively" == l3) {
      qa(f2, f2.selection.positionLineNumber);
      return true;
    }
    if ("editor.action.showContextMenu" == l3) {
      Jb(f2, 80, f2.selection.positionLineNumber * f2.lineHeight | 0);
      return true;
    }
    if ("editor.action.inlayHints.toggle" == l3) {
      t2 = d2(f2.model, f2.selection.positionLineNumber), q2 = 0;
      while (q2 < t2.length) q2++;
      setTextContent(f2.stickyWidget, emptyBuf() + (t2[q2] || "") + xf), setDisplay(f2.stickyWidget, mf);
      return true;
    }
    return false;
  }
  function r2(b2, a2) {
    var f2 = ma(zf, X(b2.model.uri), 0);
    if (0 == f2.length) return;
    for (var d3, e2 = b2.selection.positionLineNumber, g2 = f2[0].startLineNumber, c2 = 0; ; c2++) {
      if (c2 >= f2.length) {
        d3 = g2;
        break;
      }
      d3 = f2[c2].startLineNumber;
      if (!a2 && d3 > e2) break;
      a2 && d3 < e2 && (g2 = d3);
    }
    !a2 && d3 <= e2 && (d3 = f2[0].startLineNumber);
    p(b2, d3, 1), Ra(b2), m(b2);
  }
  return function(a2, b2, c2) {
    var e2, g2 = zf;
    c2 && c2.text && (g2 = c2.text + ""), c2 && c2.lineNumber && (e2 = +c2.lineNumber | 0, c2 = c2.column ? +c2.column | 0 : 1, p(a2.view, e2, c2), m(a2.view));
    if (Oc(a2.view, b2, g2)) {
      B(a2.cursorEmitter);
      return;
    }
    if (l2(a2.view, b2, g2)) {
      B(a2.cursorEmitter);
      return;
    }
    c2 = 0;
    while (c2 < a2.actions.length) {
      if (a2.actions[c2].id == b2) {
        a2.actions[c2].run(a2);
        return;
      }
      c2++;
    }
  };
})();
var Kb = /* @__PURE__ */ (function() {
  function a(a2, b2, c) {
    if (!a2) return c;
    a2 = a2[b2];
    return !a2 ? c : a2 + "" == of ? false : a2 + "" == ki ? false : true;
  }
  function b(a2, b2, c) {
    if (!a2) return c;
    a2 = a2[b2];
    return !a2 ? c : +a2 | 0;
  }
  return function(c, d2) {
    var e = W(d2, "theme", c.view.theme), f = c.view;
    f.showLineNumbers = a(d2, "lineNumbers", f.showLineNumbers), f.readOnly = a(d2, "readOnly", f.readOnly), f.tabSize = b(d2, "tabSize", f.tabSize), f.insertSpaces = a(d2, "insertSpaces", f.insertSpaces), f.fontSize = b(d2, "fontSize", f.fontSize), f.wordWrap = a(d2, "wordWrap", f.wordWrap), d2 && d2.minimap && (d2 = d2.minimap, f = c.view, f.showMinimap = a(d2, "enabled", f.showMinimap)), e != c.view.theme ? (c = c.view, c.theme = e, ba(c), m(c)) : m(c.view);
  };
})();
function bd(a, b, c, d2) {
  let e = { id: "", label: "", run: null };
  _c(e, b, c, d2), a.actions.push(e);
}
function Ta(a) {
  if (a.disposed) return;
  a.disposed = true;
  var b = a.contentEmitter;
  b.disposed = true, b.listeners = [], a = a.cursorEmitter, a.disposed = true, a.listeners = [];
}
function cd(a, b, c, d2) {
  a.root = b, a.original = c, a.modified = d2, a.changes = [];
}
function Ua(b, d2, a, c) {
  aa(), b = Dc(b, lc(d2, a)), b.theme = c, b.languageId = a, ba(b), m(b), d2 = { view: null, contentEmitter: null, cursorEmitter: null, disposed: false, actions: [], modelFacade: null }, $c(d2, b), da.push(d2), B(Ya);
  return d2;
}
function qd(a, b) {
  var d2 = W(b, nh, zf);
  d2 = Ua(a, d2, W(b, Ug, Cg), W(b, "theme", Z)), Kb(d2, b);
  if (b && b.model) {
  }
  m(d2.view);
  return d2;
}
function rd(a, b) {
  let d2 = ej, c = document.createElement(ej);
  d2 = document.createElement(d2);
  let e = Di;
  setStyle(a, Jf, Di);
  let f = "1 1 50%";
  setStyle(c, Di, f), setStyle(d2, Di, f), a.appendChild(c), a.appendChild(d2), f = W(b, "original", zf), b = W(b, Ug, Cg), c = Ua(c, f, b, Z), d2 = Ua(d2, W(b, "modified", e), b, Z), b = { root: null, original: null, modified: null, changes: [] }, cd(b, a, c, d2), Xa.push(b), B(Za);
  return b;
}
function sd() {
  return da;
}
function td() {
  return Xa;
}
function ud(a, b) {
}
function vd(a) {
  var b, c = 0;
  while (c < da.length) b = da[c].view, b.theme = a, ba(b), m(b), c++;
}
function wd(f, a) {
  f.languageId = a;
}
function xd(f, a, c) {
  var b = [];
  if (c) {
    var e, g, d2 = 0;
    while (c[d2]) b.push(pc(c[d2], a, X(f.uri))), d2 = d2 + 1 | 0;
  }
  g = X(f.uri);
  f = [], c = 0;
  while (c < G.length) d2 = G[c], e = d2.owner == a && d2.resource == g, e || f.push(d2), c++;
  c = [], d2 = 0;
  while (d2 < b.length) e = b[d2], e.owner = a, e.resource = g, c.push(e), d2++;
  a = [], b = 0;
  while (b < f.length) a.push(f[b]), b++;
  f = 0;
  while (f < c.length) a.push(c[f]), f++;
  G = a;
}
function yd(a, b, c) {
  return ma(a, b, c);
}
function zd(a) {
  var b = [], c = 0;
  while (c < G.length) G[c].owner != a && b.push(G[c]), c++;
  G = b;
}
function Ad(a, b) {
  return y(Ej, a, b);
}
function Bd(a, b) {
  return y("hover", a, b);
}
function Cd(a, b) {
  return y(Gj, a, b);
}
function Dd(a, b) {
  return y("reference", a, b);
}
function Ed(a, b) {
  return y("documentSymbol", a, b);
}
function Fd(a, b) {
  return y(Uh, a, b);
}
function Gd(a, b) {
  return y("rename", a, b);
}
function Hd(a, b) {
  return y(Vi, a, b);
}
function Id(a, b) {
  return y("folding", a, b);
}
function Jd(a, b) {
  return y("link", a, b);
}
function Kd(a, b) {
  return y(Dj, a, b);
}
function Ld(a, b) {
  return y("codeLens", a, b);
}
function Md(a, b) {
  return y(kh, a, b);
}
function Nd(a, b) {
  return y("documentHighlight", a, b);
}
function Od(a, b) {
  return y("inlayHints", a, b);
}
function Pd(a, b) {
  return y("inlineCompletions", a, b);
}
function Qd(a, b, c) {
  return y(a, b, c);
}
function Rd(d2) {
  d2 = d2.view.model.buffer;
  return I(d2, d2.root);
}
function Sd(d2, h) {
  var a = d2.view.model, c = h.replace(/\r\n|\r|\n/g, xf), e = H(c);
  h = { buffer: "", lineStarts: [] }, h.buffer = c, h.lineStarts = e, h = [h], c = { root: null, buffers: [], lineCnt: 0, length: 0, eol: "", eolLength: 0, eolNormalized: false, lastChangeBufferPos: null, cacheNode: null, cacheNodeStartOffset: 0, cacheNodeStartLineNumber: 0, cacheHasLine: false, cacheValid: false, lastVisitedLineNumber: 0, lastVisitedLineValue: "", posNode: null, posRemainder: 0, posStart: 0, walkLine: 0, walkCol: 0, tmpBuffer: null }, fa(c, h, xf, true), a.buffer = c, a.versionId = a.versionId + 1 | 0, B(a.onDidChangeContent), p(d2.view, 1, 1), m(d2.view);
}
function Td(d2) {
  return d2.view.model;
}
function Ud(d2, f) {
  d2.view.model = f, d2.view.languageId = f.languageId, p(d2.view, 1, 1), m(d2.view);
}
function Vd(d2) {
  let b = d2.view.selection.positionColumn, a = { lineNumber: 0, column: 0 };
  a.lineNumber = d2.view.selection.positionLineNumber, a.column = b;
  return a;
}
function Wd(d2, a) {
  p(d2.view, a.lineNumber, a.column), m(d2.view);
}
function Xd(d2) {
  return d2.view.selection;
}
function Yd(d2, a) {
  s(d2.view, a.selectionStartLineNumber, a.selectionStartColumn, a.positionLineNumber, a.positionColumn), m(d2.view);
}
function Zd(d2, a, b, c) {
  C(d2, b, c);
}
function _d(d2) {
  Sa(d2);
}
function $d(d2, a, b) {
  tb(d2.view, a, b);
}
function ae(d2) {
  Ta(d2);
}
function be(d2) {
  focusElement(d2.view.textarea);
}
function ce(d2, a) {
  m(d2.view);
  return o(d2.view.model, a, true);
}
function de(d2, a, b) {
  let c = d2.view.model;
  mb(c, a), a = nb(c, b), m(d2.view);
  return a;
}
function ee(d2, a) {
  sa(d2.view, a), m(d2.view);
}
function fe(d2, a, b, c) {
  bd(d2, a, b, c);
}
function ge(d2, a) {
  d2.modelFacade = a, d2.view.modelFacade = a;
}
function he(d2, a) {
  d2.contentEmitter.listeners.push(a);
  return function() {
    var g = d2.listeners.indexOf(a);
    g >= 0 && d2.listeners.splice(g, 1);
  };
}
function ie(d2, a) {
  d2.cursorEmitter.listeners.push(a);
  return function() {
    var g = d2.listeners.indexOf(a);
    g >= 0 && d2.listeners.splice(g, 1);
  };
}
function je(a) {
  let b = Ya;
  Ya.listeners.push(a);
  return function() {
    var g = b.listeners.indexOf(a);
    g >= 0 && b.listeners.splice(g, 1);
  };
}
function dd(a) {
  aa();
  if (J(a)) return;
  var b = J(Cg);
  if (b) {
    var d2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(d2, a, b.lexer, zf, zf, zf), k(d2);
  }
}
function ke(a, b) {
  var c = J(a);
  if (!c || !b) return;
  a = b.comments, a && (!a.lineComment || (c.lineComment = a.lineComment + ""), !a.blockComment || (a = a.blockComment, !a[0] || (c.blockCommentStart = a[0] + ""), !a[1] || (c.blockCommentEnd = a[1] + "")));
}
function le(a, b) {
  dd(a), a = J(a), a && (a.tokensProvider = b);
}
function me(d2, a) {
  Kb(d2, a);
}
function ne(f, a, b) {
  mb(f, a);
  return nb(f, b);
}
function oe(f) {
  return Ga(f);
}
function pe(f) {
  return Ha(f);
}
function qe(f, a) {
  f.onDidChangeContent.listeners.push(a);
  return function() {
    var g = f.listeners.indexOf(a);
    g >= 0 && f.listeners.splice(g, 1);
  };
}
function re(f, a) {
  return Aa(f.buffer, a);
}
function se(f, a) {
  f = x(n(f, a.lineNumber), a.column);
  return [f[1].toString(10), f[2].toString(10), f[0]];
}
function te(f) {
  f = f.buffer;
  return I(f, f.root);
}
function ue(f, h) {
  let b = h.replace(/\r\n|\r|\n/g, xf), c = H(b);
  h = { buffer: "", lineStarts: [] }, h.buffer = b, h.lineStarts = c, h = [h], b = { root: null, buffers: [], lineCnt: 0, length: 0, eol: "", eolLength: 0, eolNormalized: false, lastChangeBufferPos: null, cacheNode: null, cacheNodeStartOffset: 0, cacheNodeStartLineNumber: 0, cacheHasLine: false, cacheValid: false, lastVisitedLineNumber: 0, lastVisitedLineValue: "", posNode: null, posRemainder: 0, posStart: 0, walkLine: 0, walkCol: 0, tmpBuffer: null }, fa(b, h, xf, true), f.buffer = b, f.versionId = f.versionId + 1 | 0, B(f.onDidChangeContent);
}
function ve(f) {
  return u(f);
}
function we(f, a) {
  return n(f, a);
}
function xe(f, a) {
  return hostGetOffsetAt(f.buffer, a.lineNumber, a.column, l);
}
function ye(f, a) {
  return hostGetPositionAt(f.buffer, a, l);
}
function ze(f) {
  return f.languageId;
}
function Ae(f) {
  return f.versionId;
}
function Be(f) {
  return X(f.uri);
}
function Ce(f, a, b, c, d2, e) {
  return E(f.buffer, a, b, c, d2, e);
}
function De(f, a) {
  return o(f, a, true);
}
function Ee(f) {
  f = f.onDidChangeContent, f.disposed = true, f.listeners = [];
}
function Fe(d2) {
  let a = d2.original.view.model.buffer;
  d2 = d2.modified.view.model.buffer;
  return mc(I(a, a.root), I(d2, d2.root));
}
function Ge(d2) {
  Ta(d2.original), Ta(d2.modified);
}
function He(d2) {
  return d2.original;
}
function Ie(d2) {
  return d2.modified;
}
function Je(d2) {
  Sa(d2.original), Sa(d2.modified);
}
function Ke(a) {
  let b = Za;
  Za.listeners.push(a);
  return function() {
    var g = b.listeners.indexOf(a);
    g >= 0 && b.listeners.splice(g, 1);
  };
}
function Le(d2) {
  return d2.view.root;
}
function Me(d2) {
  return d2.view.widgetsHost;
}
function Ne(d2) {
  return d2.view.scrollTop;
}
function Oe(d2, a) {
  a |= 0, a < 0 && (a = 0), d2.view.scrollTop = a, m(d2.view);
}
function Pe(d2) {
  return 0;
}
function Qe(d2, h) {
}
function Re(d2) {
  d2 = d2.view;
  return (vb(d2) * d2.lineHeight | 0) + (d2.height / 2 | 0) | 0;
}
function Se(d2) {
  return d2.view.width;
}
function Te(d2) {
  let a = ub(d2.view);
  return [Ka(d2.view), 1, a, t(d2.view.model, a) + 1 | 0];
}
function Ue(d2) {
  var a = d2.view.selection;
  a = [a.selectionStartLineNumber, a.selectionStartColumn, a.positionLineNumber, a.positionColumn];
  var b, c = 0;
  while (c < d2.view.extraCursors.length) b = d2.view.extraCursors[c], a.push(b.selectionStartLineNumber), a.push(b.selectionStartColumn), a.push(b.positionLineNumber), a.push(b.positionColumn), c++;
  return a;
}
function Ve(d2, a) {
  if (a.length < 4) return;
  var b = a[0] | 0, e = a[1] | 0, f = a[2] | 0, g = a[3] | 0, c = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 };
  D(c, b, e, f, g), s(d2.view, c.selectionStartLineNumber, c.selectionStartColumn, c.positionLineNumber, c.positionColumn), m(d2.view), b = [], c = 4;
  while ((c + 3 | 0) < a.length) {
    e = a[c] | 0, f = a[c + 1 | 0] | 0, g = a[c + 2 | 0] | 0;
    var h = a[c + 3 | 0] | 0, i2 = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 };
    D(i2, e, f, g, h), b.push(i2), c = c + 4 | 0;
  }
  d2.view.extraCursors = b;
}
function We(f) {
  var a = [], b = u(f), c = 1;
  while (c <= b) a.push(n(f, c)), c = c + 1 | 0;
  return a;
}
function Xe(f) {
  return f.buffer.length;
}
function Ye(f) {
}
function Ze(f, a, e, b, d2, g, c) {
  d2 = E(f.buffer, a, d2, g, c, Kk), f = [], c = 0;
  while (c < d2.length) {
    a = d2[c];
    if (a.range.startLineNumber > e || a.range.startLineNumber == e && a.range.startColumn >= b) {
      f.push(a.range.startLineNumber), f.push(a.range.startColumn), f.push(a.range.endLineNumber), f.push(a.range.endColumn);
      return f;
    }
    c++;
  }
  d2.length > 0 && (a = d2[0], f.push(a.range.startLineNumber), f.push(a.range.startColumn), f.push(a.range.endLineNumber), f.push(a.range.endColumn));
  return f;
}
function _e(a, b) {
  let c = { lineNumber: 0, column: 0 };
  c.lineNumber = a, c.column = b;
  return c;
}
function $e(a, b, c, d2) {
  let e = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
  z(e, a, b, c, d2);
  return e;
}
function af(a, b, c, d2) {
  let e = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 };
  D(e, a, b, c, d2);
  return e;
}
var ed = /* @__PURE__ */ (function() {
  function a(a2, b2, c2, d3) {
    var e2 = { languageId: "", tokenPostfix: "", defaultToken: "", keywords: [], stateNames: [], stateRules: [], maxStack: 0 };
    S(e2, a2, b2, zf, c2), w(e2, pf), i(e2, pf, T(), 0, zf, zf, false);
    if (d3.length > 0) {
      b2 = emptyBuf(), c2 = 0;
      while (c2 < d3.length) a2 = d3.charAt(c2), c2++;
      i(e2, pf, "/" == a2 || a2 == Lj || "-" == a2 || "+" == a2 || "?" == a2 || a2 == Mj || "(" == a2 || ")" == a2 || "[" == a2 || "]" == a2 || a2 == Nj || "}" == a2 || "^" == a2 || "$" == a2 || "|" == a2 || a2 == nk ? b2 + nk + a2 : b2 + a2 + ".*", 0, If, zf, false);
    }
    a2 = false;
    i(e2, pf, Oj, 1, If, If, a2);
    var h2 = '"';
    i(e2, pf, h2, 1, rf, rf, a2), i(e2, pf, Yi, 1, rf, si, a2), i(e2, pf, Tj, 0, Hf, zf, a2), i(e2, pf, ti, 4, Nf, zf, a2), i(e2, pf, Mj, 0, zf, zf, a2), i(e2, If, Sj, 2, If, zf, a2), i(e2, If, pj, 0, If, zf, a2), i(e2, If, "\\*", 0, If, zf, a2), i(e2, rf, h2, 2, rf, zf, a2), i(e2, rf, lg, 0, rf, zf, a2), i(e2, si, Yi, 2, rf, zf, a2), i(e2, si, oj, 0, rf, zf, a2);
    return e2;
  }
  function b() {
    let d3 = "abap", f2 = mg, g2 = eg, h2 = mf, e2 = Lj, i2 = a(d3, ".abap", "abap-source abbreviated abstract accept accepting according activation actual add add-corresponding adjacent after alias aliases align all allocate alpha analysis analyzer and append appendage appending application archive area arithmetic as ascending aspect assert assign assigned assigning association asynchronous at attributes authority authority-check avg back background backup backward badi base before begin between big binary bintohex bit black blank blanks blob block blocks blue bound boundaries".split(" "), Lj), l3 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(l3, d3, i2, Lj, jg, ig), k(l3), e2 = "apex", i2 = a(e2, ".apex", [], Vf), l3 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(l3, e2, i2, Vf, jg, ig), k(l3), i2 = "azcli", l3 = a(i2, ".azcli", [], Ih);
    let m3 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(m3, i2, l3, Ih, jg, ig), k(m3), i2 = "bat", l3 = "REM", m3 = a(i2, ".bat", [], l3);
    let n3 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(n3, i2, m3, l3, jg, ig), k(n3), i2 = "bicep", l3 = a(i2, ".bicep", [], Vf), m3 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(m3, i2, l3, Vf, jg, ig), k(m3), i2 = "cameligo", m3 = of, g2 = a(i2, ".cameligo", ["abs", g2, mf, "Bytes", Ef, "Crypto", "Current", Ff, "failwith", m3, Rf, fj, pg, qg, Ig, "let%entry", "let%init", "List", Yj, "Map", hj, lh, "match%nat", ij, eh, jk, "Operation", vi, Ri, "Set", fh, "sender", "skip", Ui, "String", ug, aj, Gf, Mf, vg], Vf), h2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(h2, i2, g2, Vf, jg, ig), k(h2), d3 = "clojure", g2 = ";;", h2 = a(d3, ".clojure", [], g2), i2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(i2, d3, h2, g2, jg, ig), k(i2), d3 = "coffee", g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(g2, d3, a(d3, ".coffee", [f2, $i, Vh, "isnt", eh, pk, "yes", "@", "no", ki, Gf, m3, tg, Fi, dh, xh, Og, qg, vh, qf, wg, Bf, nf, xk, pg, Ff, hg, Rf, Cf, Rg, Jg, Of, uf, sf, Kf, mh, Rh, ug, Fh, xg, _j, vi, ok, Oh], Ih), Ih, jg, ig), k(g2);
  }
  function c() {
    let i2 = "cpp", y2 = Df, z2 = Uj, l3 = Bf, e2 = Ef, m3 = Of, A2 = Vj, f2 = Xf, n3 = nf, g2 = yf, C2 = di, F2 = of, L2 = "literal", O2 = "ref", b2 = Vf, P2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(P2, i2, a(i2, ".cpp", [y2, "amp", rj, rg, Uj, Bf, Ef, Of, Vj, sf, Xf, "constexpr", "const_cast", nf, "cpu", "decltype", yf, di, xh, Rg, yh, "dynamic_cast", "each", Ff, Sf, tj, "explicit", fg, Mg, F2, Zh, uf, uj, Rf, "friend", "gcnew", "generic", Ei, pg, qg, "initonly", zh, ji, Uf, "interior_ptr", Zf, L2, Zj, th, Bg, dh, "noexcept", "nullptr", "__nullptr", Dk, ei, "partial", "pascal", "pin_ptr", ng, Gk, Dg, gg, O2], Vf), Vf, jg, ig), k(P2), i2 = "csharp", z2 = a(i2, ".csharp", [Mg, Yh, yg, z2, "decimal", "sbyte", zi, "short", "ushort", ji, ak, Zj, "ulong", Vj, uj, yh, Bh, ni, rf, "assembly", Vh, og, O2, "out", Fi, yi, dh, Og, bk, "checked", "unchecked", yf, di, gh, Xf, pg, Ff, hg, Ef, Cf, Rg, Rf, qh, qg, Bf, nf, Ei, qf, wg, Jg, Of, uf, "lock", zg, sg, Ig, bi, "join", pk, "equals", "into", "orderby", gk, "descending"], Vf), A2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(A2, i2, z2, Vf, jg, ig), k(A2), z2 = "csp", A2 = a(z2, ".csp", [], zf), C2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(C2, z2, A2, zf, jg, ig), k(C2), z2 = "cypher", A2 = "ON", A2 = a(z2, ".cypher", "ALL AND AS ASC ASCENDING BY CALL CASE CONTAINS CREATE DELETE DESC DESCENDING DETACH DISTINCT ELSE END ENDS EXISTS IN IS LIMIT MANDATORY MATCH MERGE NOT ON ON OPTIONAL OR ORDER REMOVE RETURN SET SKIP STARTS THEN UNION UNWIND WHEN WHERE WITH XOR YIELD".split(" "), Vf), C2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(C2, z2, A2, Vf, jg, ig), k(C2), z2 = "dart", l3 = a(z2, ".dart", [y2, ni, Af, "show", og, Ff, vf, Dh, eg, Sf, qg, mh, ih, fg, Uf, hg, jh, Kf, Vh, "sync", l3, Tg, sh, Fi, Ef, "factory", xj, wg, Of, F2, dh, Gf, sf, Zh, tg, Jg, Xf, uf, pk, "typedef", nf, Rf, Dk, gh, "covariant", sk, "part", bk, yf, gj, "rethrow", Cf, "deferred", "hide", qf, vg, Rg, pg, fh, zg], Vf), m3 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(m3, z2, l3, Vf, jg, ig), k(m3), l3 = "dockerfile", m3 = a(l3, ".dockerfile", [], zf), n3 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(n3, l3, m3, zf, jg, ig), k(n3), i2 = "ecl", f2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(f2, i2, a(i2, ".ecl", ["__compressed__", "after", "all", mg, "any", og, "atmost", Ni, "beginc", "best", "between", e2, "cluster", "compressed", "compression", Xf, "counter", "csv", yf, "descend", "embed", "encoding", "encrypt", ch, "endc", yk, zk, Sf, "escape", Pi, "exclusive", "expire", fg, "extend", Xj, "few", "fileposition", "first", "flat", "forward", sg, "full", wf, "functionmacro", "group", "grouped", "heading", "hole", "ifblock", vf, qg, "inner", Uf, Zf, "joined", "keep", "keyed", "last", ah, "limit", "linkcounted", L2, "little_endian", "load"], Vf), Vf, jg, ig), k(f2), b2 = "elixir", f2 = a(b2, ".elixir", [], Ih), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(g2, b2, f2, Ih, jg, ig), k(g2);
  }
  function d2() {
    let e2 = "flow9", q2 = vf, h2 = pg, i2 = Ff, l3 = yf, f2 = a(e2, ".flow9", [q2, "require", fg, "forbid", "native", pg, Ff, "cast", "unsafe", hg, yf], Vf), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(g2, e2, f2, Vf, jg, ig), k(g2);
    let n3 = "freemarker2";
    e2 = of;
    let p2 = og;
    let o2 = a(n3, ".freemarker2", [e2, Gf, qg, og, yg], zf), s2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(s2, n3, o2, zf, jg, ig), k(s2), s2 = "fsharp", p2 = a(s2, ".fsharp", [Df, mg, "atomic", p2, eg, "asr", yi, Wf, Bf, "checked", "component", Xf, Fj, $f, nf, sf, yf, di, Rg, "done", "downcast", "downto", Ci, Ff, ch, ik, "eager", tj, Tg, Mg, e2, uf, Rf, fj, wf, "fixed", "functor", Qi, pg, qg, oi, "inherit", zh, Uf, Zf, "land", "lor", "lsl", "lsr", "lxor", "lazy", Ig, lh, "member", ij, Ah, th, Bg, "method", xj, dh, eh, tg, vi], Vf);
    let x2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(x2, s2, p2, Vf, jg, ig), k(x2), p2 = "go", h2 = a(p2, ".go", [Bf, Ef, "chan", Xf, nf, yf, "defer", Ff, Aj, Rf, "func", p2, Ei, h2, q2, Uf, hj, Tf, yj, qf, Ti, Eh, hg, Mf, gh, Uj, Gf, e2, "uint8", "uint16", "uint32", "uint64", "int8", "int16", "int32", "int64", "float32", "float64", "complex64", "complex128", zi, "rune", ak, ji, "uintptr", rf, jj], Vf), i2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(i2, p2, h2, Vf, jg, ig), k(i2), h2 = "graphql", i2 = a(h2, ".graphql", [o2, Gf, e2, "query", "mutation", "subscription", "extend", "schema", "directive", "scalar", Mf, Uf, zj, Sf, _h, Af, "fragment", pk], Ih), l3 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(l3, h2, i2, Ih, jg, ig), k(l3), h2 = "handlebars", i2 = a(h2, ".handlebars", [], zf), l3 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(l3, h2, i2, zf, jg, ig), k(l3), g2 = "hcl", e2 = a(g2, ".hcl", [x2, wj, "path", "for_each", "any", rf, Hf, Uj, Gf, e2, o2, "if ", "else ", "endif ", "for ", qg, "endfor"], Ih), f2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(f2, g2, e2, Ih, jg, ig), k(f2), e2 = "ini", f2 = a(e2, ".ini", [], Ih), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(g2, e2, f2, Ih, jg, ig), k(g2);
  }
  function e() {
    let e2 = "java", o2 = Df, p2 = nf, f2 = Rf, y2 = dh, g2 = Rg, q2 = vf, w2 = of, d3 = Vf, I2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(I2, e2, a(e2, ".java", [o2, nf, Rf, dh, hg, eg, yf, Ei, Tf, "synchronized", "boolean", Rg, pg, ng, Fi, Bf, yh, Af, Dg, wg, zi, Ff, vf, gg, "throws", Ef, Sf, vh, qf, "transient", Of, Kf, ji, "short", Jg, Vj, Zh, Uf, Dh, bk, sf, uf, Zj, "strictfp", "volatile", Xf, uj, "native", mh, Cf, Gf, w2, zg, Ri, "sealed", "non-sealed", "permits"], Vf), Vf, jg, ig), k(I2), I2 = "julia";
    let M2 = "elseif";
    y2 = a(I2, ".julia", [Wf, Cf, pg, Rf, Jg, qf, Bf, nf, wf, ai, "quote", Ig, wj, Qi, Xf, Rg, Eh, Ah, "baremodule", yg, vf, fg, ch, Ff, M2, Of, uf, th, "primitive", o2, Mf, qg, "isa", bi, y2], zf);
    let O2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(O2, I2, y2, zf, jg, ig), k(O2), y2 = "kotlin", o2 = a(y2, ".kotlin", [og, "as?", Bf, sf, nf, Rg, Ff, w2, Rf, fj, pg, qg, "!in", Uf, Vh, "!is", tg, Bh, Tf, qf, mh, Fi, wg, Gf, Jg, "typealias", "val", gh, Oh, Cf, ok, Of, $f, di, ni, "field", $g, uf, gj, vf, "init", "param", Gk, "receiver", fh, "setparam", bi, "actual", o2, "annotation", "companion", Xf, "crossinline", Bi, Sf, "expect", Tg, Zh, vj, zh, "inner", Zf, "lateinit", "noinline"], Vf), p2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(p2, y2, o2, Vf, jg, ig), k(p2), o2 = "less", p2 = a(o2, ".less", [], Vf), q2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(q2, o2, p2, Vf, jg, ig), k(q2), d3 = "lexon", o2 = "COMMENT", p2 = a(d3, ".lexon", "lexon lex clause terms contracts may pay pays appoints into to".split(" "), o2), q2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(q2, d3, p2, o2, jg, ig), k(q2), d3 = "liquid", o2 = a(d3, ".liquid", [], zf), p2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(p2, d3, o2, zf, jg, ig), k(p2), d3 = "lua", f2 = a(d3, ".lua", [mg, Bf, Rg, Ff, M2, ch, w2, f2, wf, Ei, pg, qg, wj, jj, eh, $i, Si, qf, ug, Gf, xg, Cf], Zi), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(g2, d3, f2, Zi, jg, ig), k(g2), d3 = "m3", f2 = a(d3, ".m3", "AND ANY ARRAY AS BEGIN BITS BRANDED BY CASE CONST DIV DO ELSE ELSIF END EVAL EXCEPT EXCEPTION EXIT EXPORTS FINALLY FOR FROM GENERIC IF IMPORT IN INTERFACE LOCK LOOP METHODS MOD MODULE NOT OBJECT OF OR OVERRIDES PROCEDURE RAISE RAISES READONLY RECORD REF REPEAT RETURN REVEAL SET THEN TO TRY TYPE TYPECASE UNSAFE UNTIL UNTRACED VALUE VAR WHILE WITH".split(" "), zf), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(g2, d3, f2, zf, jg, ig), k(g2);
  }
  function f() {
    let d3 = "mdx", f2 = a(d3, ".mdx", [], zf), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(g2, d3, f2, zf, jg, ig), k(g2), d3 = "mips";
    let n3 = ej, p2 = "sub", q2 = "xor";
    f2 = "lhi", f2 = a(d3, ".mips", ".data .text syscall trap add addu addi addiu and andi div divu mult multu nor or ori sll slv sra srav srl srlv sub subu xor xori lhi lho lhi llo slt slti sltu sltiu beq bgtz blez bne j jal jalr jr lb lbu lh lhu lw li la sb sh sw mfhi mflo mthi mtlo move".split(" "), Ih), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(g2, d3, f2, Ih, jg, ig), k(g2), f2 = "msdax", g2 = lk;
    let h2 = ii, i2 = xi, l3 = "DOUBLE";
    let r2 = a(f2, ".msdax", "VAR RETURN NOT EVALUATE DATATABLE ORDER BY START AT DEFINE MEASURE ASC DESC IN BOOLEAN DOUBLE INTEGER DATETIME CURRENCY STRING".split(" "), Vf), s2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(s2, f2, r2, Vf, jg, ig), k(s2), f2 = "mysql", g2 = a(f2, ".mysql", ["ACCESSIBLE", "ADD", hi, "ALTER", oh, Jh, ui, ii, "ASENSITIVE", "BEFORE", li, "BIGINT", Hi, "BLOB", wi, g2, "CALL", "CASCADE", _g, "CHANGE", "CHAR", dk, Wh, ph, wh, "CONDITION", kg, rk, "CONVERT", Lg, mj, "CUBE", "CUME_DIST", Vg, Wg, cg, Xg, "CURSOR", "DATABASE", "DATABASES", "DAY_HOUR", "DAY_MICROSECOND", "DAY_MINUTE", "DAY_SECOND", "DEC", "DECIMAL", "DECLARE", mi, "DELAYED", "DELETE", "DENSE_RANK", xi, "DESCRIBE", "DETERMINISTIC", Sg, "DISTINCTROW", "DIV", l3, "DROP", "DUAL", "EACH", Lh, "ELSEIF", "EMPTY"], Zi), h2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(h2, f2, g2, Zi, jg, ig), k(h2), r2 = "objective-c";
    let t2 = yf;
    let B2 = a(r2, ".objective-c", ["#import", "#include", "#define", "#else", "#endif", "#if", "#ifdef", "#ifndef", "#ident", "#undef", "@class", "@defs", "@dynamic", "@encode", "@end", "@implementation", "@interface", "@package", "@private", "@protected", "@property", "@protocol", "@public", "@selector", "@synthesize", "__declspec", Mi, rg, "BOOL", Bf, "bycopy", "byref", Ef, Vj, "Class", Xf, Ai, nf, yf, Rg, yh, Ff, Sf, Mg, Xh, of, uj, Rf, Ei, pg, qg, ji, "id", "inout", "IMP", Zj, jj, "nonatomic", "NULL", "oneway", "out", ng, gg, Dg], Vf), C2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(C2, r2, B2, Vf, jg, ig), k(C2), r2 = "pascal", n3 = a(r2, ".pascal", [Yf, Df, "all", "and_then", rj, og, "asm", hk, Wf, "bindable", Ef, sf, Xf, "contains", yf, n3, Ff, ch, Pi, "exports", Tg, "far", $g, "finalization", uf, "forward", "generic", Ei, pg, Af, vf, qg, "index", "inherited", "initialization", "interrupt", Vh, $h, sh, ij, Ah, "name", "near", eh, Bh, vi, pk, "only", Dk, "or_else", kk, ei, Tf, "packed", "pow", ng, "program", Dg, gg, "published", Uf, "implementation", "qualified", "read"], Vf), t2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(t2, r2, n3, Vf, jg, ig), k(t2), n3 = "pascaligo", h2 = a(n3, ".pascaligo", [B2, mf, h2, Xf, Ff, ch, Xj, Rf, sg, wf, pg, Vh, jj, vi, "remove", qf, "skip", ug, Mf, gh, Cf, vg, Ch, Rj, "transaction"], Vf), i2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(i2, n3, h2, Vf, jg, ig), k(i2), d3 = "perl", f2 = a(d3, ".perl", ["__DATA__", f2, "lock", "__END__", sj, "lt", uk, "eq", vk, "exp", "ne", p2, "__PACKAGE__", Rf, "no", mg, qh, $i, Fh, "cmp", "ge", Tf, xg, nf, "gt", Cf, "CORE", pg, q2, Rg, "le", "__DIE__", "__WARN__"], Ih), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(g2, d3, f2, Ih, jg, ig), k(g2);
  }
  function g() {
    let b2 = "pgsql", e2 = Zi, f2 = a(b2, ".pgsql", [hi, "ANALYSE", oh, Jh, bj, kj, ui, ii, "ASYMMETRIC", Pg, Hi, wi, _g, Pj, Wh, ph, ek, wh, "CONCURRENTLY", kg, Lg, mj, "CURRENT_CATALOG", Vg, "CURRENT_ROLE", "CURRENT_SCHEMA", Wg, cg, Xg, mi, Cj, xi, Sg, "DO", Lh, cj, Ii, Xh, "FETCH", "FOR", "FOREIGN", "FREEZE", Qj, "FULL", "GRANT", "GROUP", "HAVING", "ILIKE", mk, "INITIALLY", "INNER", "INTERSECT", "INTO", "IS", "ISNULL", "JOIN", "LATERAL", "LEADING", "LEFT", "LIKE", "LIMIT", "LOCALTIME", "LOCALTIMESTAMP", "NATURAL"], Zi), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(g2, b2, f2, Zi, jg, ig), k(g2), e2 = "php", f2 = a(e2, ".php", [], Vf), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(g2, e2, f2, Vf, jg, ig), k(g2), f2 = "pla", g2 = a(f2, ".pla", ".i .o .mv .ilb .ob .label .type .phase .pair .symbolic .symbolic-output .kiss .p .e .end".split(" "), Ih);
    let h2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(h2, f2, g2, Ih, jg, ig), k(h2);
    let i2 = "postiats", l3 = og, m3 = Wf, o2 = Rg;
    let q2 = ug;
    let t2 = a(i2, ".postiats", "abstype abst0ype absprop absview absvtype absviewtype absvt0ype absviewt0ype as and assume begin case classdec datasort datatype dataprop dataview datavtype dataviewtype do end extern extype extvar exception fn fnx fun prfn prfun praxi castfn if then else ifcase in infix infixl infixr prefix postfix implmnt implement primplmnt primplement import lam llam fix let local macdef macrodef nonfix symelim symintr overload of op rec sif scase".split(" "), Vf), u2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(u2, i2, t2, Vf, jg, ig), k(u2), i2 = "powerquery", t2 = "each", u2 = of, q2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(q2, i2, a(i2, ".powerquery", [l3, t2, Ff, "error", u2, pg, qg, Vh, Ig, "meta", kk, "section", "shared", ug, Gf, Jg, Mf], Vf), Vf, jg, ig), k(q2), i2 = "powershell", m3 = a(i2, ".powershell", [m3, Bf, Of, sf, nf, Bi, "define", Rg, "dynamicparam", Ff, "elseif", ch, Wj, "filter", uf, Rf, qh, sg, wf, pg, qg, "param", "process", qf, hg, wg, "trap", Jg, xg, yg, gh, Cf, "workflow", "parallel", "sequence", "inlinescript", "configuration"], Ih), o2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(o2, i2, m3, Ih, jg, ig), k(o2), e2 = "protobuf", i2 = Tf, i2 = a(e2, ".protobuf", ["syntax", vf, "weak", gg, i2, Ch, "repeated", "oneof", hj, "reserved", aj, "max", Sf, pi, "service", "rpc", "stream", "returns", i2, Ek, Gf, u2], Vf), m3 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(m3, e2, i2, Vf, jg, ig), k(m3), e2 = "pug", f2 = a(e2, ".pug", [Li, mf, Ef, yf, "doctype", t2, Ff, Kf, Rf, f2, qg, oi, xj, Og, Fh, gh, Oh], Vf), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(g2, e2, f2, Vf, jg, ig), k(g2);
  }
  function h() {
    let d3 = "qsharp", r2 = wf, e2 = pg, f2 = Ff, t2 = Si, u2 = xg, o2 = "self", z2 = "use", p2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(p2, d3, a(d3, ".qsharp", [Bg, "open", vf, fg, og, jk, wf, "body", "adjoint", "newtype", Eh, "controlled", pg, Ci, Ff, Si, xg, "fixup", Rf, qg, Cf, qf, Xj, "within", "apply", "Adjoint", "Controlled", "Adj", "Ctl", Vh, o2, rg, "distribute", "invert", "intrinsic", Ig, fh, "w/", dh, eh, mg, $i, z2, "borrow", yg, "borrowing", th, Zf], Vf), Vf, jg, ig), k(p2), d3 = "r", p2 = Bf;
    let A2 = "next";
    r2 = a(d3, ".r", [p2, A2, qf, pg, Ff, Rf, qg, Si, Cf, rj, "category", "character", "complex", yh, r2, "integer", Yj, "logical", "matrix", "numeric", "vector", "data.frame", "factor", sh, "require", "attach", "detach", Ui], Ih), t2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(t2, d3, r2, Ih, jg, ig), k(t2), r2 = "razor", t2 = a(r2, ".razor", [], zf);
    let B2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(B2, r2, t2, zf, jg, ig), k(B2), r2 = "redis", t2 = a(r2, ".redis", "APPEND AUTH BGREWRITEAOF BGSAVE BITCOUNT BITFIELD BITOP BITPOS BLPOP BRPOP BRPOPLPUSH CLIENT KILL LIST GETNAME PAUSE REPLY SETNAME CLUSTER ADDSLOTS COUNT-FAILURE-REPORTS COUNTKEYSINSLOT DELSLOTS FAILOVER FORGET GETKEYSINSLOT INFO KEYSLOT MEET NODES REPLICATE RESET SAVECONFIG SET-CONFIG-EPOCH SETSLOT SLAVES SLOTS COMMAND COUNT GETKEYS CONFIG GET REWRITE SET RESETSTAT DBSIZE DEBUG OBJECT SEGFAULT DECR DECRBY DEL DISCARD DUMP ECHO EVAL EVALSHA EXEC EXISTS EXPIRE EXPIREAT FLUSHALL FLUSHDB GEOADD".split(" "), zf), B2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(B2, r2, t2, zf, jg, ig), k(B2), r2 = "redshift";
    let C2 = a(r2, ".redshift", ["AES128", "AES256", hi, "ALLOWOVERWRITE", "ANALYSE", oh, Jh, bj, kj, ui, ii, Pg, "AZ64", "BACKUP", li, Hi, "BLANKSASNULL", wi, "BYTEDICT", "BZIP2", _g, Pj, Wh, ph, wh, kg, Lg, "CREDENTIALS", mj, Vg, Wg, cg, Xg, "CURRENT_USER_ID", mi, Cj, "DEFLATE", "DEFRAG", "DELTA", "DELTA32K", xi, "DISABLE", Sg, "DO", Lh, "EMPTYASNULL", "ENABLE", "ENCODE", "ENCRYPT", "ENCRYPTION", cj, Ii, "EXPLICIT", Xh, "FOR", "FOREIGN", "FREEZE", Qj, "FULL", "GLOBALDICT256", "GLOBALDICT64K", "GRANT", "GROUP", "GZIP"], Zi), D2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(D2, r2, C2, Zi, jg, ig), k(D2), r2 = "restructuredtext", B2 = a(r2, ".restructuredtext", [], zf), C2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(C2, r2, B2, zf, jg, ig), k(C2), d3 = "ruby", B2 = of, t2 = a(d3, ".ruby", [vk, "__ENCODING__", uk, lj, t2, Yh, mg, Wf, p2, Ef, sf, "def", "defined?", Rg, Ff, sj, ch, "ensure", Rf, B2, pg, qg, Ah, A2, jj, eh, $i, "redo", "rescue", "retry", qf, o2, mh, ug, Gf, "undef", Fh, xg, Oh, Cf, zg], Ih), u2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(u2, d3, t2, Ih, jg, ig), k(u2), d3 = "rust", f2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(f2, d3, a(d3, ".rust", [og, ih, jh, "box", p2, Xf, nf, "crate", "dyn", Ff, Sf, Mg, B2, "fn", Rf, e2, "impl", qg, Ig, _j, lh, ij, $j, "mut", "pub", "ref", qf, o2, Dh, Eh, mh, "trait", Gf, Jg, Mf, "unsafe", z2, bi, Cf, Of, yf, zj, Dh, Df, "alignof", "become", Rg, Zh, ai, "offsetof", ei, "priv", "proc", "pure", "sizeof", Og, "unsized", "virtual", zg], Vf), Vf, jg, ig), k(f2);
  }
  function l2() {
    let d3 = "sb", e2 = Yi, f2 = a(d3, ".sb", "Else ElseIf EndFor EndIf EndSub EndWhile For Goto If Step Sub Then To While".split(" "), Yi), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(g2, d3, f2, Yi, jg, ig), k(g2), g2 = "scala";
    let h2 = Rg;
    let l3 = Rf;
    let s2 = ug;
    let i2 = a(g2, ".scala", ["asInstanceOf", Of, sf, "classOf", "def", Rg, Ff, Kf, uf, Rf, qh, "forSome", pg, vf, "isInstanceOf", ai, lh, dh, Bh, Tf, qf, wg, "trait", Jg, Mf, xg, "val", gh, Cf, vg, zg, "given", Sf, ug], Vf), t2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(t2, g2, i2, Vf, jg, ig), k(t2), g2 = "scheme";
    let v2 = "load";
    i2 = ";";
    let w2 = a(g2, ".scheme", [Ef, Rg, Ig, _j, pg, Ff, Oh, "cons", "car", "cdr", "cond", "lambda", "lambda*", "syntax-rules", "format", "set!", "quote", "eval", Li, Yj, "list?", "member?", v2], i2), x2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(x2, g2, w2, i2, jg, ig), k(x2), g2 = "scss", i2 = a(g2, ".scss", [], Vf), w2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(w2, g2, i2, Vf, jg, ig), k(w2), w2 = "shell", h2 = a(w2, ".shell", [f2, ug, h2, Ff, Ci, Cf, xg, Rf, qg, "esac", "fi", "fin", "fil", "done", Wj, fh, "unset", fg, wf], Ih), l3 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(l3, w2, h2, Ih, jg, ig), k(l3), h2 = "solidity", l3 = "contract", s2 = "address";
    let B2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(B2, h2, a(h2, ".solidity", "pragma solidity contract library using struct function modifier constructor address string bool Int Uint Byte Fixed Ufixed int int8 int16 int24 int32 int40 int48 int56 int64 int72 int80 int88 int96 int104 int112 int120 int128 int136 int144 int152 int160 int168 int176 int184 int192 int200 int208 int216 int224 int232 int240 int248 int256 uint uint8 uint16 uint24 uint32 uint40 uint48 uint56 uint64 uint72 uint80 uint88 uint96 uint104".split(" "), Vf), Vf, jg, ig), k(B2), h2 = "sophia", B2 = of, e2 = a(h2, ".sophia", [l3, sh, "entrypoint", wf, "stateful", "state", "hash", "signature", "tuple", Yj, s2, rf, Uj, ji, Ri, wk, Mf, Ch, "oracle", "oracle_query", "Call", "Bits", "Bytes", "Oracle", "String", "Crypto", "Address", "Auth", "Chain", Rj, "Some", "bits", "bytes", tj, Ig, hj, ng, gg, Gf, B2, gh, f2, e2, wg], Vf), f2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(f2, h2, e2, Vf, jg, ig), k(f2), d3 = "sparql", e2 = a(d3, ".sparql", ["add", og, "asc", "ask", yi, ok, "clear", "construct", Ai, "create", Bi, xh, "desc", "describe", "distinct", "drop", B2, "filter", sg, "graph", "group", "having", qg, "insert", "limit", v2, "minus", $j, "named", eh, "offset", Ek, "order", "prefix", "reduced", Ti, "service", "silent", aj, Gf, "undef", zj, yg, "values", bi, vg], Ih), f2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(f2, d3, e2, Ih, jg, ig), k(f2);
  }
  function m2() {
    let d3 = "sql", e2 = Zi, f2 = a(d3, ".sql", ["ABORT", "ABSOLUTE", "ACTION", "ADA", "ADD", "AFTER", hi, "ALLOCATE", "ALTER", "ALWAYS", oh, Jh, bj, "ARE", ui, ii, "ASSERTION", "AT", "ATTACH", Pg, "AUTOINCREMENT", "AVG", "BACKUP", "BEFORE", lj, li, "BIT", "BIT_LENGTH", wi, "BREAK", "BROWSE", "BULK", lk, "CASCADE", "CASCADED", _g, Pj, "CATALOG", "CHAR", dk, "CHARACTER_LENGTH", "CHAR_LENGTH", Wh, "CHECKPOINT", "CLOSE", "CLUSTERED", "COALESCE", ph, ek, wh, "COMMIT", "COMPUTE", "CONFLICT", "CONNECT", "CONNECTION", kg, "CONSTRAINTS", qk, "CONTAINSTABLE", rk, "CONVERT", "CORRESPONDING", "COUNT", Lg], Zi), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(g2, d3, f2, Zi, jg, ig), k(g2);
    let m3 = "st";
    g2 = pg;
    let h2 = Ef;
    let p2 = a(m3, ".st", [g2, "end_if", sj, Ff, Ef, vi, aj, "__try", "__catch", "__finally", Rg, vg, ok, Cf, Si, "end_while", "end_repeat", "end_case", Rf, "end_for", "task", "retain", "non_retain", "constant", vg, "at", Wj, qf, "interval", "priority", "address", "port", "on_channel", ug, "iec", $g, "uses", "version", "packagetype", "displayname", "copyright", "summary", "vendor", "common_source", sg, Kf, Af], Vf), q2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(q2, m3, p2, Vf, jg, ig), k(q2), m3 = "swift";
    let t2 = of, w2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(w2, m3, a(m3, ".swift", ["#available", "#colorLiteral", "#column", "#dsohandle", "#else", "#elseif", "#endif", "#error", "#file", "#fileID", "#fileLiteral", "#filePath", "#function", "#if", "#imageLiteral", "#keyPath", "#line", "#selector", "#sourceLocation", "#warning", "Any", "Protocol", "Self", "Type", "actor", og, "assignment", "associatedtype", "associativity", ih, jh, Bf, Ef, Of, sf, nf, "convenience", yf, "defer", "deinit", "didSet", Rg, ni, "dynamicType", Ff, Sf, "extension", Aj, t2, "fileprivate", Zh, Rf, "func", gj, "guard", "higherThan", g2, vf, qg, "indirect", vj, "init", "inout", Zf], Vf), Vf, jg, ig), k(w2), m3 = "systemverilog", f2 = a(m3, ".systemverilog", "accept_on alias always always_comb always_ff always_latch and assert assign assume automatic before begin bind bins binsof bit break buf bufif0 bufif1 byte case casex casez cell chandle checker class clocking cmos config const constraint context continue cover covergroup coverpoint cross deassign default defparam design disable dist do edge else end endcase endchecker endclass endclocking endconfig endfunction endgenerate endgroup endinterface endmodule endpackage endprimitive endprogram endproperty".split(" "), Vf), h2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(h2, m3, f2, Vf, jg, ig), k(h2), h2 = "tcl", m3 = a(h2, ".tcl", [], zf), p2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(p2, h2, m3, zf, jg, ig), k(p2), h2 = "twig", e2 = a(h2, ".twig", ["apply", "autoescape", mf, "deprecated", e2, "embed", Kf, "flush", Rf, sg, g2, vf, oi, ai, "sandbox", fh, "use", "verbatim", vg, "endapply", "endautoescape", "endblock", yk, "endfor", "endif", zk, "endsandbox", "endset", "endwith", Gf, t2], zf), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(g2, h2, e2, zf, jg, ig), k(g2), e2 = "typespec", f2 = a(e2, ".typespec", [], Vf), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(g2, e2, f2, Vf, jg, ig), k(g2), d3 = "vb", f2 = a(d3, ".vb", "AddHandler AddressOf Alias And AndAlso As Async Boolean ByRef Byte ByVal Call Case Catch CBool CByte CChar CDate CDbl CDec Char CInt Class CLng CObj Const Continue CSByte CShort CSng CStr CType CUInt CULng CUShort Date Decimal Declare Default Delegate Dim DirectCast Do Double Each Else ElseIf End EndIf Enum Erase Error Event Exit False Finally For Friend Function Get GetType GetXMLNamespace Global GoSub".split(" "), Yi), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(g2, d3, f2, Yi, jg, ig), k(g2);
  }
  function n2() {
    let d3 = "wgsl", e2 = Vf, f2 = a(d3, ".wgsl", [], Vf), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    j(g2, d3, f2, Vf, jg, ig), k(g2), d3 = "xml", f2 = a(d3, ".xml", [], zf), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(g2, d3, f2, zf, jg, ig), k(g2), d3 = "yaml", e2 = "Null", e2 = a(d3, ".yaml", [Gf, "True", "TRUE", of, nj, Xh, tg, e2, e2, "~"], Ih), g2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, j(g2, d3, e2, Ih, jg, ig), k(g2);
  }
  return function() {
    if (_a) return;
    _a = true, b(), c(), d2(), e(), f(), g(), h(), l2(), m2(), n2();
  };
})();
function bf(d2, a, b, c, e) {
  return E(d2.view.model.buffer, a, b, c, e, Kk);
}
function cf(d2) {
  return pb(d2.view.model);
}
function df(d2, a) {
  return la(d2.view.model, a);
}
function ef(d2, a) {
  return x(n(d2.view.model, a.lineNumber), a.column)[0];
}
function ff(d2, a) {
  return qb(d2.view.model, a);
}
function gf(d2) {
  sb(d2.view.model, d2.view.selection.positionLineNumber, d2.view.languageId), m(d2.view);
}
function hf(d2, e) {
  e |= 0;
  var a = u(d2.view.model);
  e < 1 && (e = 1), e > a || (a = e), p(d2.view, a, 1), sa(d2.view, a), m(d2.view);
}
function Lb() {
  aa(), ed();
}
function jf(a, g) {
  Lb(), a = Ja(a);
  return !a ? [] : ob(a, g);
}
function kf(c, g) {
  Lb(), c = Ja(c);
  if (!c) return [];
  g = ob(c, g);
  var a = [];
  for (c = 0; c < g.length; c++) a.push(g[c].offset.toString(10) + ":" + g[c].type);
  return a;
}
function Q(a, b, c, d2) {
  let e = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
  z(e, a, b, c, d2), b = concat2(emptyBuf(), zf), a = { range: null, text: "", identifier: 0 }, a.range = e, a.text = b, a.identifier = 0;
  return a;
}
var Mk = { bufferIndex: 0, start: { line: 0, column: 0 }, end: { line: 0, column: 0 }, lineFeedCnt: 0, length: 0 };
var Nk = { parent: null, left: null, right: null, color: 0, piece: null, size_left: 0, lf_left: 0, alive: false };
ya(Nk, Mk, 0);
var l = Nk;
var wa = 1;
var Ok = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
z(Ok, 1, 1, 1, 1);
Mk = { id: 0, range: null, className: "", hoverMessage: "", isWholeLine: false };
Mk.id = 0;
Mk.range = Ok;
Mk.className = Mk.hoverMessage = zf;
Mk.isWholeLine = false;
Nk = { parent: null, left: null, right: null, color: 0, deco: null, maxEndLine: 0, maxEndColumn: 0, alive: false };
jb(Nk, Mk);
var r = Nk;
var xa = [];
var F = [];
var Va = false;
var fd = 1;
var Mb = 2;
var Nb = 4;
var Wa = 8;
var G = [];
var L = [];
var da = [];
var Xa = [];
Mk = { listeners: [], disposed: false };
Mk.listeners = [];
Mk.disposed = false;
var Ya = Mk;
Mk = { listeners: [], disposed: false };
Mk.listeners = [];
Mk.disposed = false;
var Za = Mk;
var _a = false;
var gd = 2048;
var hd = 1024;
var id = 512;
var jd = 256;

// apps/monaco/samples.js
var WORKSPACE = "demo-ide";
var FILES = [
  {
    path: "README.md",
    language: "markdown",
    value: `# demo-ide

Two served editors, same chrome:

- LilScript: compiled Lil editor (piece tree, Monarch, textarea + minimap canvas)
- JS: npm monaco-editor 0.56 (VS Code editor + JSON/CSS/HTML/TS workers)

Open files from the explorer. Ctrl/Cmd+P quick-opens. Ctrl/Cmd+F finds in the current file.
`
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
`
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
`
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
`
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
    <script type="module" src="./src/main.ts"><\/script>
  </body>
</html>
`
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
`
  },
  {
    path: "data.json",
    language: "json",
    value: `{
  "editor": "monaco",
  "versions": ["0.56.0"],
  "features": ["models", "tabs", "find", "markers"]
}
`
  }
];

// apps/monaco/workbench.js
function fileUri(monaco2, path) {
  const raw = "file:///" + WORKSPACE + "/" + path;
  if (monaco2.Uri?.parse) {
    return monaco2.Uri.parse(raw);
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
function markerRows(monaco2) {
  const rows = monaco2.editor.getModelMarkers?.({}) ?? [];
  return rows.map((m2) => ({
    message: m2.message ?? "",
    severity: Number(m2.severity ?? 8),
    startLineNumber: Number(m2.startLineNumber ?? 1),
    startColumn: Number(m2.startColumn ?? 1),
    resource: String(m2.resource?.path ?? m2.resource?.toString?.() ?? m2.resource ?? "")
  })).filter((m2) => m2.message);
}
function severityLabel(severity) {
  if (severity >= 8) return "error";
  if (severity >= 4) return "warning";
  if (severity >= 2) return "info";
  return "hint";
}
function mountIde(monaco2, options) {
  const root = document.getElementById("app");
  const hasTs = Boolean(monaco2.languages?.typescript);
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
          <button class="active" data-panel="files" title="Explorer">\u2630</button>
          <button data-panel="search" title="Search">\u2315</button>
          <button data-panel="problems" title="Problems">\u26A0</button>
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
    const ts = monaco2.languages.typescript;
    ts.typescriptDefaults.setCompilerOptions({
      target: ts.ScriptTarget.ES2020,
      module: ts.ModuleKind.ESNext,
      moduleResolution: ts.ModuleResolutionKind.NodeJs,
      allowNonTsExtensions: true,
      noEmit: true,
      strict: true
    });
    ts.javascriptDefaults.setCompilerOptions({
      allowNonTsExtensions: true,
      noEmit: true,
      checkJs: true
    });
  }
  const models = /* @__PURE__ */ new Map();
  for (const file of FILES) {
    const uri = fileUri(monaco2, file.path);
    const existing = monaco2.editor.getModel?.(uri);
    const model = existing ?? monaco2.editor.createModel(file.value, file.language, uri);
    models.set(file.path, { file, model, dirty: false });
  }
  const host = document.getElementById("editor");
  const wrap = document.getElementById("editor-wrap");
  const first = models.get("src/app.ts");
  const editor = monaco2.editor.create(host, {
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
    contextmenu: true
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
    monaco2.editor.setModelMarkers(first.model, "demo", [
      {
        startLineNumber: 9,
        startColumn: 14,
        endLineNumber: 9,
        endColumn: 20,
        message: "Type 'string' is not assignable to type 'number' (sample marker; Lil has no tsc worker).",
        severity: monaco2.MarkerSeverity?.Error ?? 8
      }
    ]);
    const keywords = ["const", "export", "function", "import", "return", "string", "number"];
    for (const lang of ["typescript", "javascript"]) {
      monaco2.languages.registerCompletionItemProvider(lang, {
        provideCompletionItems(model, position) {
          const word = model.getWordAtPosition?.(position);
          const start = word?.startColumn ?? position.column;
          const end = word?.endColumn ?? position.column;
          const range = {
            startLineNumber: position.lineNumber,
            startColumn: start,
            endLineNumber: position.lineNumber,
            endColumn: end
          };
          return {
            suggestions: keywords.map((name) => ({
              label: name,
              kind: monaco2.languages.CompletionItemKind?.Keyword ?? 17,
              insertText: name,
              range
            }))
          };
        }
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
    const q2 = filter.trim().toLowerCase();
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
      for (const marker of markerRows(monaco2)) {
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
      if (q2 && !file.path.toLowerCase().includes(q2)) {
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
      btn.textContent = (entry?.dirty ? "\u25CF " : "") + path.split("/").pop();
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
    const i2 = openTabs.indexOf(path);
    if (i2 >= 0) {
      openTabs.splice(i2, 1);
    }
    if (!openTabs.length) {
      openTabs.push("README.md");
    }
    if (activePath === path) {
      openFile(openTabs[Math.max(0, i2 - 1)] ?? openTabs[0]);
    } else {
      render();
    }
  }
  function renderProblems() {
    const panel = document.getElementById("problems");
    const rows = markerRows(monaco2);
    panel.textContent = rows.length ? rows.map((m2) => `${severityLabel(m2.severity).padEnd(7)} ${m2.resource}:${m2.startLineNumber}  ${m2.message}`).join("\n") : "No problems detected.";
  }
  function renderStatus() {
    const pos = readPosition(editor);
    const lang = editor.getModel?.()?.getLanguageId?.() ?? "";
    document.getElementById("status-left").textContent = `${WORKSPACE}  ${activePath}`;
    document.getElementById("status-right").textContent = `Ln ${pos.lineNumber}, Col ${pos.column}   ${lang}   UTF-8`;
  }
  function render() {
    document.getElementById("side-title").textContent = sidePanel === "search" ? "Search" : sidePanel === "problems" ? "Problems" : "Explorer";
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
    const q2 = query.trim().toLowerCase();
    hits.innerHTML = "";
    for (const file of FILES) {
      if (q2 && !file.path.toLowerCase().includes(q2)) {
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
  monaco2.editor.onDidChangeMarkers?.(() => {
    renderProblems();
    if (sidePanel === "problems") renderFiles();
  });
  render();
  openFile("src/app.ts");
  return editor;
}

// apps/monaco/lil/ide-entry.js
var monaco = bindMonaco(entry_raw_exports);
globalThis.monaco = monaco;
mountIde(monaco, {
  label: "LilScript",
  otherHref: "../js/",
  otherLabel: "JS monaco-editor \u2192",
  banner: "LilScript compiled editor: piece-tree model, Monarch highlighting, textarea + canvas minimap. Not VS Code workbench, not tsc/ts.worker."
});
