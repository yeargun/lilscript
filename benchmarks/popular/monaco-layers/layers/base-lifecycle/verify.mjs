import assert from "node:assert/strict";

export async function verify(lil, js) {
  const store = lil.createDisposableStore();
  let n = 0;
  lil.storeAdd(store, () => {
    n++;
  });
  lil.storeDispose(store);
  assert.equal(n, 1);

  const em = lil.createEmitter();
  let fired = 0;
  const sub = lil.emitterEvent(em, () => {
    fired++;
  });
  lil.emitterFire(em);
  assert.equal(fired, 1);
  sub();
  lil.emitterFire(em);
  assert.equal(fired, 1);

  const uri = lil.parseUri("file:///tmp/a.ts");
  assert.equal(uri.scheme, "file");
  const jsUri = js.URI.parse("file:///tmp/a.ts");
  assert.equal(jsUri.scheme, "file");
  assert.equal(lil.fileUri("/x").scheme, "file");
  assert.equal(lil.inmemoryUri().scheme, "inmemory");
  assert.equal(lil.keyCodeFromKey("Enter"), 3);
  assert.equal(js.KeyCode.Enter, 3);
}
