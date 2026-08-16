function replaceFunction(source, signature, replacement) {
  const start = source.indexOf(signature);
  if (start < 0) throw new Error(`Missing reactive function: ${signature}`);
  const signatureOpen = signature.indexOf("{");
  const open =
    signatureOpen >= 0
      ? start + signatureOpen
      : source.indexOf("{", start + signature.length);
  let depth = 0;
  let quote = "";
  let escaped = false;
  for (let index = open; index < source.length; index += 1) {
    const character = source[index];
    if (quote) {
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === quote) quote = "";
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
    } else if (character === "{") depth += 1;
    else if (character === "}" && --depth === 0) {
      return source.slice(0, start) + replacement + source.slice(index + 1);
    }
  }
  throw new Error(`Unclosed reactive function: ${signature}`);
}

// When the closed call graph has effects but no memo/resource producers, all
// pending computations are ordinary effects. The dependency-level scheduler
// then specializes to FIFO without changing the observable effect semantics.
export function createEffectOnlyReactiveSource(source) {
  source = replaceFunction(
    source,
    "  T read() {",
    `  T read() {
    if (activeListenerId >= 0) {
      int effectId = activeListenerId;
      if (this.subscribe(effectId)) {
        effects[effectId].sources.push(JS.assume(this));
      }
    }
    return this.value;
  }`,
  );
  source = source.replace(
    /    if \(activeEffectId >= 0 && effects\[activeEffectId\]\.memoComputation\) \{\n      this\.level = effects\[activeEffectId\]\.level;\n      this\.producerEffectId = activeEffectId;\n    \}\n/,
    "",
  );
  source = replaceFunction(
    source,
    "void queueEffect(int effectId) {",
    `void queueEffect(int effectId) {
  if (!effects[effectId].disposed && !effects[effectId].queued) {
    effects[effectId].queued = true;
    pendingEffects.push(effectId);
  }
}`,
  );
  source = replaceFunction(
    source,
    "int takeNextEffect() {",
    `int takeNextEffect() {
  return takeQueuedEffectAt(0);
}`,
  );
  source = replaceFunction(
    source,
    "void flushPureEffects() {",
    `void flushPureEffects() {
  flushEffects();
}`,
  );
  source = replaceFunction(
    source,
    "void scheduleObservers(int[] observerIds) {",
    `void scheduleObservers(int[] observerIds) {
  for (int index = 0; index < observerIds.length; index++) {
    queueEffect(observerIds[index]);
  }
  if (batchDepth == 0) {
    flushEffects();
  }
}`,
  );
  return source
    .replaceAll('"Reactive dependency cycle detected."', '""')
    .replaceAll('"Circular memo dependency detected."', '""')
    .replaceAll('"Potential Infinite Loop Detected."', '""');
}
