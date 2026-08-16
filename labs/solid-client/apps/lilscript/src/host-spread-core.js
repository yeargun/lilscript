const xlinkNamespace = "http://www.w3.org/1999/xlink";
const xmlNamespace = "http://www.w3.org/XML/1998/namespace";
const booleanProperties = new Set([
  "checked",
  "disabled",
  "multiple",
  "muted",
  "readOnly",
  "required",
  "selected",
]);
const stringProperties = new Set(["value"]);

function classList(node, value, previous = {}) {
  value ??= {};
  for (const name of Object.keys(previous)) {
    if (!value[name]) {
      for (const token of name.trim().split(/\s+/))
        if (token) node.classList.toggle(token, false);
      delete previous[name];
    }
  }
  for (const name of Object.keys(value)) {
    const enabled = Boolean(value[name]);
    if (previous[name] === enabled) continue;
    for (const token of name.trim().split(/\s+/))
      if (token) node.classList.toggle(token, enabled);
    previous[name] = enabled;
  }
  return previous;
}

function style(node, value, previous = {}) {
  if (typeof value === "string") {
    node.style.cssText = value;
    return value;
  }
  if (typeof previous === "string") {
    node.style.cssText = "";
    previous = {};
  }
  value ??= {};
  for (const name of Object.keys(previous)) {
    if (value[name] == null) node.style.removeProperty(name);
  }
  for (const name of Object.keys(value)) {
    if (previous[name] !== value[name])
      node.style.setProperty(name, value[name]);
  }
  return { ...value };
}

function assign(node, name, value, previous, isSvg) {
  if (name === "children") return value;
  if (name === "ref") return value;
  if (name === "classList") return classList(node, value, previous);
  if (name === "style") return style(node, value, previous);
  if (/^on[A-Z]/.test(name)) {
    const event = name.slice(2).toLowerCase();
    if (previous !== value) {
      if (typeof previous === "function")
        node.removeEventListener(event, previous);
      if (typeof value === "function") node.addEventListener(event, value);
    }
    return value;
  }
  if (name === "className") name = "class";
  if (name === "htmlFor") name = "for";
  if (booleanProperties.has(name)) {
    node[name] = Boolean(value);
    return value;
  }
  if (stringProperties.has(name)) {
    node[name] = value == null ? "" : String(value);
    return value;
  }
  const separator = isSvg ? name.indexOf(":") : -1;
  const namespace =
    separator > 0
      ? name.slice(0, separator) === "xlink"
        ? xlinkNamespace
        : name.slice(0, separator) === "xml"
          ? xmlNamespace
          : null
      : null;
  if (namespace) {
    if (value == null || value === false)
      node.removeAttributeNS(namespace, name.slice(separator + 1));
    else node.setAttributeNS(namespace, name, String(value));
  } else if (value == null || value === false) {
    node.removeAttribute(name);
  } else {
    node.setAttribute(name, value === true ? "" : String(value));
  }
  return value;
}

export function installSpreadHost(scope, nodes) {
  scope.domSpread = (handle, props, previous, isSvg) => {
    const node = nodes[handle];
    for (const name of Object.keys(previous)) {
      if (!(name in props)) {
        assign(node, name, null, previous[name], isSvg);
        delete previous[name];
      }
    }
    for (const name of Object.keys(props)) {
      // JSX children are materialized by the renderer. Reading a children
      // getter here would create a second, detached subtree on every spread
      // update even though assign() deliberately ignores the property.
      if (name === "children" || name === "ref") continue;
      const value = props[name];
      if (
        previous[name] === value &&
        name !== "classList" &&
        name !== "style"
      ) {
        continue;
      }
      previous[name] = assign(node, name, value, previous[name], isSvg);
    }
    return previous;
  };
}
