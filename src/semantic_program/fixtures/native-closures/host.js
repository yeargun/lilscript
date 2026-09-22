// Equivalent source callbacks. Allocation checks belong only to native memory
// qualification and deliberately produce no source-visible events here.
const slots = new Array(8).fill(null);
globalThis.keep = (slot, callback) => { slots[slot] = callback; };
globalThis.invoke = (slot, delta) => {
  const callback = slots[slot];
  if (callback === null) throw new Error("invoking empty slot");
  return callback(delta);
};
globalThis.clear = slot => { slots[slot] = null; };
globalThis.clearAll = () => { slots.fill(null); };
globalThis.take = slot => {
  const callback = slots[slot];
  if (callback === null) throw new Error("taking empty slot");
  slots[slot] = null;
  return callback;
};
globalThis.expectLive = _count => {};
globalThis.assertEmpty = () => {};
