import { installPortalHost } from "./host-portals-core.js";
import { eventHosts, nodes, store } from "./host-state.js";

installPortalHost(globalThis, nodes, eventHosts);
globalThis.domIsHead = (node) => nodes[node] === document.head;
globalThis.domAttachShadow = (node) =>
  store(nodes[node].attachShadow({ mode: "open" }));
