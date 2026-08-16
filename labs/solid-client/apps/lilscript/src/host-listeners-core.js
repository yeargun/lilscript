export function installListenerHost(scope, nodes, listeners, freeListeners) {
  scope.domAddEventListener = (node, event, callback) => {
    const listener = freeListeners.pop() ?? listeners.length;
    const target = nodes[node];
    listeners[listener] = { node: target, event, callback };
    target.addEventListener(event, callback);
    return listener;
  };

  scope.domRemoveEventListener = (listener) => {
    const entry = listeners[listener];
    if (entry === undefined) return;
    entry.node.removeEventListener(entry.event, entry.callback);
    listeners[listener] = undefined;
    freeListeners.push(listener);
  };
}
