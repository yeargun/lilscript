import {
  installElementHost,
  installTemplateHost,
} from "../apps/lilscript/src/host-elements-core.js";
import { installDelegatedEventHost } from "../apps/lilscript/src/host-events-core.js";
import { installListenerHost } from "../apps/lilscript/src/host-listeners-core.js";
import { installPortalHost } from "../apps/lilscript/src/host-portals-core.js";
import { installSpreadHost } from "../apps/lilscript/src/host-spread-core.js";
import { reconcileDomNodes } from "../packages/solidlil/internal/reconcile-dom.js";

export function installDomHost(window) {
  const nodes = [];
  const freeNodes = [];
  const listeners = [];
  const freeListeners = [];
  const handles = new WeakMap();
  const eventHosts = new WeakMap();
  const store = (node) => {
    const existing = handles.get(node);
    if (existing !== undefined) return existing;
    const id = freeNodes.pop() ?? nodes.length;
    nodes[id] = node;
    handles.set(node, id);
    return id;
  };
  const release = (node) => {
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
  };

  installElementHost(window, window.document, store);
  installTemplateHost(window, window.document, store, (id) => nodes[id]);
  installListenerHost(window, nodes, listeners, freeListeners);
  installDelegatedEventHost(
    window,
    window.document,
    nodes,
    handles,
    (node) => eventHosts.get(node) ?? node.parentNode ?? node.host,
  );
  installPortalHost(window, nodes, eventHosts);
  installSpreadHost(window, nodes);

  window.domQueryRoot = (selector) =>
    store(window.document.querySelector(selector));
  window.domCreateElement = (tag) => store(window.document.createElement(tag));
  window.domCreateText = (value) =>
    store(window.document.createTextNode(value));
  window.domCreateComment = () => store(window.document.createComment(""));
  window.domCreateFragment = () =>
    store(window.document.createDocumentFragment());
  window.domChildNodes = (parent) => [...nodes[parent].childNodes].map(store);
  window.domIsFragment = (node) =>
    nodes[node]?.nodeType === window.Node.DOCUMENT_FRAGMENT_NODE;
  window.domReleaseNode = (node) => {
    const value = nodes[node];
    if (value) release(value);
  };
  window.domAppendChild = (parent, child) =>
    nodes[parent].appendChild(nodes[child]);
  window.domRemoveNode = (node) => {
    const value = nodes[node];
    if (!value) return;
    value.remove();
    release(value);
  };
  window.domReconcileOne = (parent, marker, current, next) => {
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
  window.domReconcile = (parent, marker, current, next) => {
    const markerNode = nodes[marker];
    const parentNode = markerNode?.parentNode ?? nodes[parent];
    reconcileDomNodes(
      parentNode,
      markerNode,
      current.map((id) => nodes[id]).filter(Boolean),
      next.map((id) => nodes[id]).filter(Boolean),
      release,
    );
  };
  window.domSetText = (node, value) => {
    nodes[node].data = value;
  };
  window.domSetAttribute = (node, name, value) => {
    nodes[node].setAttribute(name, value);
  };
  window.domSetAttributeNS = (node, namespace, name, value) => {
    nodes[node].setAttributeNS(namespace, name, value);
  };
  window.domIsHead = (node) => nodes[node] === window.document.head;
  window.domAttachShadow = (node) =>
    store(nodes[node].attachShadow({ mode: "open" }));
  window.domSetBoolAttribute = (node, name, value) => {
    if (value) nodes[node].setAttribute(name, "");
    else nodes[node].removeAttribute(name);
  };
  window.domSetStringProperty = (node, name, value) => {
    nodes[node][name] = value;
  };
  window.domSetBoolProperty = (node, name, value) => {
    nodes[node][name] = value;
  };
  window.domToggleClass = (node, name, value) => {
    for (const token of name.trim().split(/\s+/)) {
      if (token) nodes[node].classList.toggle(token, value);
    }
  };
  window.domSetStyleProperty = (node, name, value) => {
    if (value === "") nodes[node].style.removeProperty(name);
    else nodes[node].style.setProperty(name, value);
  };
  window.domClear = (node) => {
    const parent = nodes[node];
    for (let child = parent.firstChild; child;) {
      const next = child.nextSibling;
      release(child);
      child = next;
    }
    parent.replaceChildren();
  };
  window.hostSchedule = (callback) => callback();
  window.registerBenchmarkDispose = (callback) => {
    window.__disposeSolidBenchmark = callback;
  };
}
