import { installSpreadHost } from "./host-spread-core.js";
import { nodes } from "./host-state.js";

installSpreadHost(globalThis, nodes);
