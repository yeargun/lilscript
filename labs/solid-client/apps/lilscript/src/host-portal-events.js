import { installDelegatedEventHost } from "./host-events-core.js";
import { eventHosts, handles, nodes } from "./host-state.js";

installDelegatedEventHost(
  globalThis,
  document,
  nodes,
  handles,
  (node) => eventHosts.get(node) ?? node.parentNode ?? node.host,
);
