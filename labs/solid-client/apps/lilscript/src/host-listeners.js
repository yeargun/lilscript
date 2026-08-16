import { freeListeners, listeners, nodes } from "./host-state.js";
import { installListenerHost } from "./host-listeners-core.js";

installListenerHost(globalThis, nodes, listeners, freeListeners);
