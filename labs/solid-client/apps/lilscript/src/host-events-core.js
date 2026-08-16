export function installDelegatedEventHost(
  scope,
  document,
  nodes,
  handles,
  parentFor,
) {
  let delegatedHandlers = new WeakMap();
  const delegatedRoots = new Map();
  const events = [];
  const registrations = [];
  const freeRegistrations = [];

  function dispatch(event) {
    let target = event.target;
    if (event.composed && target?.shadowRoot) {
      target = event.composedPath?.()?.[0] ?? target;
    }
    let eventId = -1;
    let state;

    try {
      const visit = (node) => {
        const entry = delegatedHandlers.get(node)?.get(event.type);
        if (entry !== undefined && !node.disabled) {
          if (entry.usesEvent) {
            if (state === undefined) {
              eventId = events.length;
              state = { current: node, event, target };
              events.push(state);
            } else {
              state.current = node;
            }
            entry.callback(eventId);
          } else {
            entry.callback();
          }
        }
        return !event.cancelBubble;
      };

      let node = target;
      while (node && node !== document && visit(node)) {
        node = parentFor(node);
      }
    } finally {
      if (state !== undefined) events.pop();
    }
  }

  function addDelegatedEvent(node, event, callback, usesEvent) {
    const target = nodes[node];
    let handlers = delegatedHandlers.get(target);
    if (handlers === undefined) {
      handlers = new Map();
      delegatedHandlers.set(target, handlers);
    }

    const registration = freeRegistrations.pop() ?? registrations.length;
    const previous = handlers.get(event);
    const entry = { callback, event, registration, target, usesEvent };
    handlers.set(event, entry);
    registrations[registration] = entry;

    if (previous === undefined) {
      const root = delegatedRoots.get(event);
      if (root === undefined) {
        const listener = (value) => dispatch(value);
        delegatedRoots.set(event, { count: 1, listener });
        document.addEventListener(event, listener);
      } else {
        root.count += 1;
      }
    }
    return registration;
  }

  scope.domAddDelegatedEvent = (node, event, callback) =>
    addDelegatedEvent(node, event, callback, true);
  scope.domAddDelegatedEventVoid = (node, event, callback) =>
    addDelegatedEvent(node, event, callback, false);
  scope.domAddDelegatedClickVoid = (node, callback) =>
    addDelegatedEvent(node, "click", callback, false);
  scope.domSetDelegatedClickVoid = (node, callback) => {
    addDelegatedEvent(node, "click", callback, false);
  };

  scope.domRemoveDelegatedEvent = (registration) => {
    const entry = registrations[registration];
    if (entry === undefined) return;
    registrations[registration] = undefined;
    freeRegistrations.push(registration);

    const handlers = delegatedHandlers.get(entry.target);
    if (handlers?.get(entry.event) !== entry) return;
    handlers.delete(entry.event);
    if (handlers.size === 0) delegatedHandlers.delete(entry.target);

    const root = delegatedRoots.get(entry.event);
    root.count -= 1;
    if (root.count === 0) {
      document.removeEventListener(entry.event, root.listener);
      delegatedRoots.delete(entry.event);
    }
  };
  scope.domRemoveDelegatedClick = scope.domRemoveDelegatedEvent;

  scope.domClearDelegatedEvents = () => {
    for (const [event, root] of delegatedRoots) {
      document.removeEventListener(event, root.listener);
    }
    delegatedRoots.clear();
    delegatedHandlers = new WeakMap();
  };

  scope.domEventTarget = (event) => handles.get(events[event].target) ?? -1;
  scope.domEventCurrentTarget = (event) =>
    handles.get(events[event].current) ?? -1;
  scope.domEventType = (event) => events[event].event.type;
  scope.domEventDefaultPrevented = (event) =>
    events[event].event.defaultPrevented;
  scope.domEventPreventDefault = (event) =>
    events[event].event.preventDefault();
  scope.domEventStopPropagation = (event) =>
    events[event].event.stopPropagation();
}
