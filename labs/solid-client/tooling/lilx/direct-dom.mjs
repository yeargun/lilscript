function classRange(source, name) {
  const start = source.indexOf(`export class ${name}`);
  if (start < 0) throw new Error(`Missing ${name} class`);
  const open = source.indexOf("{", start);
  let depth = 0;
  for (let index = open; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    else if (source[index] === "}" && --depth === 0) {
      return { start, end: index + 1 };
    }
  }
  throw new Error(`Unclosed ${name} class`);
}

function eraseDomNodeConstruction(source) {
  const prefix = "new DomNode(";
  let output = "";
  let cursor = 0;
  while (true) {
    const start = source.indexOf(prefix, cursor);
    if (start < 0) return output + source.slice(cursor);
    output += source.slice(cursor, start);
    let depth = 1;
    let quote = "";
    let escaped = false;
    let end = start + prefix.length;
    for (; end < source.length; end += 1) {
      const character = source[end];
      if (quote) {
        if (escaped) escaped = false;
        else if (character === "\\") escaped = true;
        else if (character === quote) quote = "";
      } else if (character === '"' || character === "'") quote = character;
      else if (character === "(") depth += 1;
      else if (character === ")" && --depth === 0) break;
    }
    if (depth !== 0) throw new Error("Unclosed DomNode construction");
    output += source.slice(start + prefix.length, end);
    cursor = end + 1;
  }
}

// Closed-world DOM nodes are transparent host handles. This is equivalent to
// newtype erasure: source-level DomNode typing remains available to LSX users,
// while the linked browser runtime stores the underlying Node directly.
export function createDirectDomWebSource(
  source,
  { errorBoundary = true, suspense = true } = {},
) {
  const eventRange = classRange(source, "DomEvent");
  let eventClass = source.slice(eventRange.start, eventRange.end);
  source =
    source.slice(0, eventRange.start) +
    "__LILX_DOM_EVENT_CLASS__" +
    source.slice(eventRange.end);

  const nodeRange = classRange(source, "DomNode");
  source = source.slice(0, nodeRange.start) + source.slice(nodeRange.end);
  source = eraseDomNodeConstruction(source)
    .replaceAll(".id", "")
    .replace(/\bDomNode\b/g, "JsValue")
    .replace(
      `JsValue[] currentIds = [];
    for (int index = 0; index < this.current.length; index++) {
      currentIds.push(this.current[index]);
    }
    JsValue[] nextIds = [];
    for (int index = 0; index < next.length; index++) {
      nextIds.push(next[index]);
    }
    domReconcile(this.parent, this.marker, currentIds, nextIds);`,
      "domReconcile(this.parent, this.marker, this.current, next);",
    );
  eventClass = eraseDomNodeConstruction(eventClass).replace(
    /\bDomNode\b/g,
    "JsValue",
  );
  source = source.replace("__LILX_DOM_EVENT_CLASS__", eventClass);
  if (source.includes("currentIds.push(this.current[index])")) {
    throw new Error("Failed to elide DomRegion identity copies");
  }
  if (!errorBoundary) {
    source = source.replace(
      /export int createRenderEffect\(func\(\)->void callback\) \{[\s\S]*?\n\}/,
      `export int createRenderEffect(func()->void callback) {
  return createReactiveRenderEffect(callback);
}`,
    );
  }
  if (!suspense) {
    source = source.replace(
      /enableSuspenseResolution\(\(\) => useContext\(suspenseContext\)\);\s*/,
      "",
    );
  }
  return source;
}
