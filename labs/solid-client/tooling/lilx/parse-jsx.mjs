const VOID = new Set([
  "area",
  "base",
  "br",
  "col",
  "embed",
  "hr",
  "img",
  "input",
  "link",
  "meta",
  "param",
  "source",
  "track",
  "wbr",
]);

const DELEGATED = new Set([
  "click",
  "dblclick",
  "mousedown",
  "mouseup",
  "input",
  "change",
  "submit",
  "keydown",
  "keyup",
  "focusin",
  "focusout",
]);

export function parseJsx(input, start = 0) {
  const parser = new Parser(input, start);
  const node = parser.parseElement();
  return { node, end: parser.index };
}

class Parser {
  constructor(input, index) {
    this.input = input;
    this.index = index;
  }

  peek() {
    return this.input[this.index];
  }

  eof() {
    return this.index >= this.input.length;
  }

  skipWs() {
    while (!this.eof() && /\s/.test(this.peek())) this.index += 1;
  }

  parseElement() {
    this.skipWs();
    if (this.peek() !== "<") throw this.error("expected <");
    this.index += 1;
    if (this.peek() === "/") throw this.error("unexpected closing tag");
    const tag = this.parseTagName();
    const props = [];
    this.skipWs();
    while (!this.eof() && this.peek() !== ">" && this.peek() !== "/") {
      props.push(this.parseProp());
      this.skipWs();
    }
    if (this.peek() === "/") {
      this.index += 1;
      if (this.peek() !== ">") throw this.error("expected >");
      this.index += 1;
      return { type: "element", tag, props, children: [], selfClosing: true };
    }
    if (this.peek() !== ">") throw this.error("expected >");
    this.index += 1;
    if (VOID.has(tag)) {
      return { type: "element", tag, props, children: [], selfClosing: true };
    }
    return {
      type: "element",
      tag,
      props,
      children: this.parseChildren(tag),
      selfClosing: false,
    };
  }

  parseChildren(openTag) {
    const children = [];
    while (!this.eof()) {
      if (this.input.startsWith("</", this.index)) {
        this.index += 2;
        const close = this.parseTagName();
        this.skipWs();
        if (this.peek() !== ">") throw this.error("expected >");
        this.index += 1;
        if (close !== openTag) throw this.error(`mismatched </${close}>`);
        return children;
      }
      if (this.peek() === "<") {
        children.push(this.parseElement());
        continue;
      }
      if (this.peek() === "{") {
        children.push({ type: "expr", value: this.parseBrace() });
        continue;
      }
      const value = this.parseText();
      if (value.trim()) children.push({ type: "text", value });
    }
    throw this.error(`unclosed <${openTag}>`);
  }

  parseText() {
    const start = this.index;
    while (!this.eof() && this.peek() !== "<" && this.peek() !== "{") {
      this.index += 1;
    }
    return this.input.slice(start, this.index);
  }

  parseTagName() {
    const start = this.index;
    if (!/[A-Za-z]/.test(this.peek())) throw this.error("expected tag name");
    this.index += 1;
    while (!this.eof() && /[A-Za-z0-9._-]/.test(this.peek())) {
      this.index += 1;
    }
    return this.input.slice(start, this.index);
  }

  parseProp() {
    if (this.peek() === "{") {
      const value = this.parseBrace();
      if (value.startsWith("...")) {
        return { type: "spread", value: value.slice(3).trim() };
      }
      throw this.error("unsupported bare {} prop");
    }
    const name = this.parsePropName();
    this.skipWs();
    if (this.peek() !== "=") return { type: "bool", name, value: true };
    this.index += 1;
    this.skipWs();
    if (this.peek() === "{") {
      return { type: "expr", name, value: this.parseBrace() };
    }
    if (this.peek() === '"' || this.peek() === "'") {
      const quote = this.peek();
      this.index += 1;
      const start = this.index;
      while (!this.eof() && this.peek() !== quote) this.index += 1;
      if (this.eof()) throw this.error(`unclosed ${quote} attribute`);
      const value = this.input.slice(start, this.index);
      this.index += 1;
      return { type: "string", name, value };
    }
    throw this.error("expected prop value");
  }

  parsePropName() {
    const start = this.index;
    if (!/[A-Za-z_]/.test(this.peek())) throw this.error("expected prop name");
    this.index += 1;
    while (!this.eof() && /[A-Za-z0-9_:-]/.test(this.peek())) {
      this.index += 1;
    }
    return this.input.slice(start, this.index);
  }

  parseBrace() {
    if (this.peek() !== "{") throw this.error("expected {");
    this.index += 1;
    const start = this.index;
    let depth = 1;
    let quote = null;
    while (!this.eof()) {
      const character = this.peek();
      if (quote) {
        if (character === "\\") {
          this.index += 2;
          continue;
        }
        if (character === quote) quote = null;
        this.index += 1;
        continue;
      }
      if (character === '"' || character === "'" || character === "`") {
        quote = character;
        this.index += 1;
        continue;
      }
      if (character === "{") depth += 1;
      if (character === "}") {
        depth -= 1;
        if (depth === 0) {
          const value = this.input.slice(start, this.index).trim();
          this.index += 1;
          return value;
        }
      }
      this.index += 1;
    }
    throw this.error("unclosed {");
  }

  error(message) {
    return new Error(
      `${message} at ${this.index}: ${this.input.slice(this.index, this.index + 40)}`,
    );
  }
}

export function isComponent(tag) {
  return /^[A-Z]/.test(tag);
}

export function eventName(property) {
  if (!/^on[A-Z]/.test(property)) return null;
  return property.slice(2).toLowerCase();
}

export function isDelegated(name) {
  return DELEGATED.has(name);
}
