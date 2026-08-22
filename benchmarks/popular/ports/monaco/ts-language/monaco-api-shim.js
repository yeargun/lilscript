class Emitter {
  constructor() {
    this._listeners = [];
  }

  get event() {
    return (listener) => {
      this._listeners.push(listener);
      return {
        dispose: () => {
          this._listeners = this._listeners.filter((fn) => fn !== listener);
        },
      };
    };
  }

  fire(value) {
    for (const listener of this._listeners.slice()) {
      listener(value);
    }
  }
}

class Range {
  constructor(startLineNumber, startColumn, endLineNumber, endColumn) {
    this.startLineNumber = startLineNumber;
    this.startColumn = startColumn;
    this.endLineNumber = endLineNumber;
    this.endColumn = endColumn;
  }

  static isIRange(value) {
    return Boolean(
      value &&
        typeof value.startLineNumber === "number" &&
        typeof value.startColumn === "number",
    );
  }
}

function monaco() {
  return globalThis.monaco;
}

function live(ns) {
  return new Proxy(
    {},
    {
      get(_target, key) {
        return monaco()?.[ns]?.[key];
      },
      set(_target, key, value) {
        const root = monaco()?.[ns];
        if (root) root[key] = value;
        return true;
      },
    },
  );
}

const MarkerTag = { Unnecessary: 1, Deprecated: 2 };
const MarkerSeverity = {
  Hint: 1,
  Info: 2,
  Warning: 4,
  Error: 8,
};

export const languages = live("languages");
export const editor = live("editor");
export const Uri = new Proxy(function UriShim() {}, {
  get(_target, key) {
    return monaco()?.Uri?.[key];
  },
  apply(_target, _thisArg, args) {
    return monaco()?.Uri?.(...args);
  },
});

export { Emitter, Range, MarkerTag, MarkerSeverity };
