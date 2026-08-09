function escapeHtml(value) {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export function serializeElement(node) {
  if (node.nodeType === 3) {
    return escapeHtml(node.data);
  }
  if (node.nodeType !== 1) {
    return "";
  }
  const attrs = [];
  for (let i = 0; i < node.attributes.length; i += 1) {
    const attr = node.attributes[i];
    if (attr.name === "key") continue;
    attrs.push([attr.name, attr.value]);
  }
  attrs.sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
  let out = `<${node.localName}`;
  for (let i = 0; i < attrs.length; i += 1) {
    out += ` ${attrs[i][0]}="${escapeHtml(attrs[i][1])}"`;
  }
  out += ">";
  for (let child = node.firstChild; child; child = child.nextSibling) {
    out += serializeElement(child);
  }
  out += `</${node.localName}>`;
  return out;
}
