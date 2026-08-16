export const nodes = [];
export const freeNodes = [];
export const listeners = [];
export const freeListeners = [];

export const handles = new WeakMap();
export const eventHosts = new WeakMap();

export function store(node) {
  const existing = handles.get(node);
  if (existing !== undefined) return existing;
  const id = freeNodes.pop() ?? nodes.length;
  nodes[id] = node;
  handles.set(node, id);
  return id;
}

export function resolveNodes(ids) {
  const result = [];
  for (const id of ids) {
    const node = nodes[id];
    if (node) result.push(node);
  }
  return result;
}

export function release(node) {
  for (let child = node.firstChild; child;) {
    const next = child.nextSibling;
    release(child);
    child = next;
  }
  if (node.shadowRoot) release(node.shadowRoot);
  const id = handles.get(node);
  if (id === undefined) return;
  nodes[id] = undefined;
  handles.delete(node);
  freeNodes.push(id);
}
