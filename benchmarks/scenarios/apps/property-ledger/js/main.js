let digest = 0;
for (let index = 0; index < 5000; index += 1) {
  digest += globalThis.consume({
    _descriptiveCount: index % 17,
    _descriptiveWeight: index % 13,
    _descriptiveOffset: index % 7,
  });
}
console.log(`property-ledger:${digest}`);
