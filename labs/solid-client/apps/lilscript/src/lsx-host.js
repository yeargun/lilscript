import "./host.js";
import "./host-elements.js";
import "./host-properties.js";
import "./host-listeners.js";
import "./host-portal-events.js";
import "./host-regions.js";
import "./host-portals.js";
import "./host-spread.js";
import { freeNodes, nodes } from "./host-state.js";

globalThis.registerLsxDispose = (callback) => {
  globalThis.__disposeLsx = callback;
};
globalThis.registerLsxDiagnostics = (
  ownerSlots,
  effectSlots,
  freeOwnerSlots,
  freeEffectSlots,
  pendingEffects,
) => {
  globalThis.__lsxDiagnostics = () => ({
    owners: ownerSlots(),
    effects: effectSlots(),
    freeOwners: freeOwnerSlots(),
    freeEffects: freeEffectSlots(),
    pendingEffects: pendingEffects(),
    nodeSlots: nodes.length,
    freeNodeSlots: freeNodes.length,
    occupiedNodeSlots: nodes.reduce(
      (count, node) => count + Number(node !== undefined),
      0,
    ),
    detachedNodeSlots: nodes.reduce(
      (count, node) => count + Number(node !== undefined && !node.isConnected),
      0,
    ),
  });
};
