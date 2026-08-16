import { handles, nodes } from "./host-state.js";
import { installDelegatedEventHost } from "./host-events-core.js";

installDelegatedEventHost(
  globalThis,
  document,
  nodes,
  handles,
  (node) => node.parentNode ?? node.host,
);
