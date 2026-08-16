import { nodes, release, resolveNodes, store } from "./host-state.js";
import { reconcileDomNodes } from "../../../packages/solidlil/internal/reconcile-dom.js";

globalThis.domCreateComment = () => store(document.createComment(""));
globalThis.domCreateFragment = () => store(document.createDocumentFragment());
globalThis.domChildNodes = (parent) => [...nodes[parent].childNodes].map(store);
globalThis.domIsFragment = (node) =>
  nodes[node]?.nodeType === document.DOCUMENT_FRAGMENT_NODE;
globalThis.domReleaseNode = (node) => {
  const value = nodes[node];
  if (value) release(value);
};
globalThis.domRemoveNode = (node) => {
  const value = nodes[node];
  if (!value) return;
  value.remove();
  release(value);
};
globalThis.domReconcileOne = (parent, marker, current, next) => {
  if (current === next) return;
  const currentNode = nodes[current];
  const nextNode = nodes[next];
  if (!nextNode) return;
  if (!currentNode) {
    const parentNode = nodes[marker]?.parentNode ?? nodes[parent];
    parentNode.insertBefore(nextNode, nodes[marker]);
    return;
  }
  const parentNode =
    currentNode.parentNode ?? nodes[marker]?.parentNode ?? nodes[parent];
  parentNode.replaceChild(nextNode, currentNode);
  release(currentNode);
};
globalThis.domReconcile = (parent, marker, current, next) => {
  const markerNode = nodes[marker];
  const parentNode = markerNode?.parentNode ?? nodes[parent];
  reconcileDomNodes(
    parentNode,
    markerNode,
    resolveNodes(current),
    resolveNodes(next),
    release,
  );
};
