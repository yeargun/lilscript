import { createWebWorker } from "./worker-client.js";

const SOLID_LIB = `
declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T | ((prev: T) => T)) => T;
  export function createSignal<T>(value: T): [Accessor<T>, Setter<T>];
  export function createMemo<T>(fn: () => T): Accessor<T>;
  export function createEffect(fn: () => void): void;
  export function onMount(fn: () => void): void;
  export function onCleanup(fn: () => void): void;
  export function Show(props: { when: unknown; fallback?: unknown; children?: unknown }): unknown;
  export function For<T>(props: { each: T[]; children?: (item: T, index?: Accessor<number>) => unknown }): unknown;
}
declare module "solid-js/web" {
  export function render(code: () => unknown, element: Element): () => void;
}
declare module "solid-js/jsx-runtime" {
  export namespace JSX {
    interface IntrinsicElements {
      [elemName: string]: any;
    }
    type Element = any;
    type ElementChildrenAttribute = { children: {} };
  }
}
declare namespace JSX {
  interface IntrinsicElements {
    [elemName: string]: any;
  }
  type Element = any;
}
`;

function disposable(fn) {
  return { dispose: typeof fn === "function" ? fn : () => {} };
}

function wordUntil(model, position) {
  const word = model.getWordAtPosition?.(position);
  const column = position?.column ?? 1;
  if (!word) {
    return { word: "", startColumn: column, endColumn: column };
  }
  const take = Math.max(0, column - word.startColumn);
  return {
    word: String(word.word ?? "").slice(0, take),
    startColumn: word.startColumn,
    endColumn: column,
  };
}

function enhanceModel(model) {
  if (!model || model.__tsReady) return model;
  model.__tsReady = true;
  if (typeof model.getWordUntilPosition !== "function") {
    model.getWordUntilPosition = (position) => wordUntil(model, position);
  }
  if (typeof model.isDisposed !== "function") {
    model.isDisposed = () => Boolean(model.__disposed);
  }
  if (typeof model.isAttachedToEditor !== "function") {
    model.isAttachedToEditor = () => true;
  }
  if (typeof model.isTooLargeForSyncing !== "function") {
    model.isTooLargeForSyncing = () => false;
  }
  if (typeof model.onDidChangeAttached !== "function") {
    model.onDidChangeAttached = () => disposable();
  }
  if (typeof model.onWillDispose !== "function") {
    const orig = typeof model.dispose === "function" ? model.dispose.bind(model) : () => {};
    const listeners = [];
    model.onWillDispose = (listener) => {
      listeners.push(listener);
      return disposable(() => {
        const i = listeners.indexOf(listener);
        if (i >= 0) listeners.splice(i, 1);
      });
    };
    model.dispose = () => {
      model.__disposed = true;
      for (const listener of listeners.slice()) listener(model);
      return orig();
    };
  }
  if (typeof model.getOptions !== "function") {
    model.getOptions = () => ({ tabSize: 2, insertSpaces: true });
  }
  return model;
}

function syncPaintMarkers(monaco, model) {
  if (!model) return;
  const rows = monaco.editor.getModelMarkers?.({ resource: model.uri }) ?? [];
  model._paintMarkers = rows.map((m) => ({
    line: m.startLineNumber,
    sc: m.startColumn,
    ec: m.endColumn,
    severity: Number(m.severity ?? 8),
  }));
}

function patchMonacoApi(monaco) {
  const modelCreated = [];
  const modelDisposed = [];
  const modelLanguage = [];
  const languageWaiters = new Map();

  const languages = monaco.languages;
  languages.CompletionItemKind = languages.CompletionItemKind ?? monaco.CompletionItemKind ?? {
    Method: 0,
    Function: 1,
    Constructor: 2,
    Field: 3,
    Variable: 4,
    Class: 5,
    Interface: 7,
    Module: 8,
    Property: 9,
    Enum: 15,
    Keyword: 17,
    File: 20,
  };
  languages.CompletionItemTag = { Deprecated: 1 };
  languages.SignatureHelpTriggerKind = { Invoke: 1, TriggerCharacter: 2, ContentChange: 3 };
  languages.DocumentHighlightKind = { Text: 0, Read: 1, Write: 2 };
  languages.InlayHintKind = { Type: 1, Parameter: 2 };
  languages.SymbolKind = languages.SymbolKind ?? {
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
    TypeParameter: 25,
  };

  if (typeof languages.onLanguage !== "function") {
    languages.onLanguage = (languageId, callback) => {
      const ids = (languages.getLanguages?.() ?? []).map((row) => row.id);
      if (ids.includes(languageId)) {
        queueMicrotask(callback);
        return disposable();
      }
      const list = languageWaiters.get(languageId) ?? [];
      list.push(callback);
      languageWaiters.set(languageId, list);
      return disposable(() => {
        languageWaiters.set(
          languageId,
          (languageWaiters.get(languageId) ?? []).filter((fn) => fn !== callback),
        );
      });
    };
  }
  languages.registerOnTypeFormattingEditProvider = (selector, provider) =>
    languages.registerDocumentFormattingEditProvider(selector, provider);
  if (typeof languages.registerDocumentRangeFormattingEditProvider !== "function") {
    languages.registerDocumentRangeFormattingEditProvider = (selector, provider) => {
      return languages.registerDocumentFormattingEditProvider(selector, {
        provideDocumentFormattingEdits(model, options, token) {
          const range = model.getFullModelRange?.() ?? {
            startLineNumber: 1,
            startColumn: 1,
            endLineNumber: model.getLineCount?.() ?? 1,
            endColumn: (model.getLineLength?.(model.getLineCount?.() ?? 1) ?? 0) + 1,
          };
          return provider.provideDocumentRangeFormattingEdits(model, range, options, token);
        },
        provideDocumentRangeFormattingEdits: provider.provideDocumentRangeFormattingEdits?.bind(provider),
      });
    };
  }

  const origCreate = monaco.editor.createModel.bind(monaco.editor);
  monaco.editor.createModel = (value, language, uri) => {
    const model = enhanceModel(origCreate(value, language, uri));
    for (const listener of modelCreated.slice()) listener(model);
    return model;
  };
  const origGetModels = monaco.editor.getModels.bind(monaco.editor);
  monaco.editor.getModels = () => origGetModels().map(enhanceModel);
  const origGetModel = monaco.editor.getModel.bind(monaco.editor);
  monaco.editor.getModel = (uri) => {
    const model = origGetModel(uri);
    return model ? enhanceModel(model) : model;
  };
  const origSetMarkers = monaco.editor.setModelMarkers.bind(monaco.editor);
  monaco.editor.setModelMarkers = (model, owner, markers) => {
    origSetMarkers(model, owner, markers);
    syncPaintMarkers(monaco, model);
  };

  monaco.editor.onDidCreateModel = (listener) => {
    modelCreated.push(listener);
    return disposable(() => {
      const i = modelCreated.indexOf(listener);
      if (i >= 0) modelCreated.splice(i, 1);
    });
  };
  monaco.editor.onWillDisposeModel = (listener) => {
    modelDisposed.push(listener);
    return disposable(() => {
      const i = modelDisposed.indexOf(listener);
      if (i >= 0) modelDisposed.splice(i, 1);
    });
  };
  monaco.editor.onDidChangeModelLanguage = (listener) => {
    modelLanguage.push(listener);
    return disposable(() => {
      const i = modelLanguage.indexOf(listener);
      if (i >= 0) modelLanguage.splice(i, 1);
    });
  };
  monaco.editor.createWebWorker = (opts) => createWebWorker(opts);

  monaco.MarkerTag = { Unnecessary: 1, Deprecated: 2 };
}

export async function attachTypescript(monaco) {
  patchMonacoApi(monaco);
  const ts = await import("monaco-ts-register");
  monaco.languages.typescript = ts;
  monaco.typescript = ts;
  ts.typescriptDefaults?.setEagerModelSync?.(true);
  ts.javascriptDefaults?.setEagerModelSync?.(true);
  ts.typescriptDefaults?.setCompilerOptions?.({
    target: ts.ScriptTarget?.ES2020 ?? 7,
    module: ts.ModuleKind?.ESNext ?? 99,
    moduleResolution: ts.ModuleResolutionKind?.NodeJs ?? 2,
    jsx: ts.JsxEmit?.Preserve ?? 1,
    jsxImportSource: "solid-js",
    allowNonTsExtensions: true,
    allowImportingTsExtensions: true,
    allowJs: true,
    noEmit: true,
    strict: true,
  });
  ts.javascriptDefaults?.setCompilerOptions?.({
    allowNonTsExtensions: true,
    allowJs: true,
    checkJs: true,
    jsx: ts.JsxEmit?.Preserve ?? 1,
    noEmit: true,
  });
  ts.typescriptDefaults?.addExtraLib?.(SOLID_LIB, "ts:solid-js.d.ts");
  return ts;
}

export { SOLID_LIB };
