import { nodes, store } from "./host-state.js";
import {
  installElementHost,
  installTemplateHost,
} from "./host-elements-core.js";

installElementHost(globalThis, document, store);
installTemplateHost(globalThis, document, store, (id) => nodes[id]);
globalThis.domSetAttributeNS = (node, namespace, name, value) => {
  nodes[node].setAttributeNS(namespace, name, value);
};
