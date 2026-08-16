export function installPortalHost(scope, nodes, eventHosts) {
  scope.domSetEventHost = (node, host) => {
    eventHosts.set(nodes[node], nodes[host]);
  };
}
