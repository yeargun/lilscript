function smaller(left, right) {
  return left < right ? left : right;
}

function larger(left, right) {
  return left > right ? left : right;
}

function beginLedger(value) {
  return { _total: value, _weighted: value, _minimum: value, _maximum: value };
}

function appendLedger(ledger, value, index) {
  return {
    _total: ledger._total + value | 0,
    _weighted: ledger._weighted + (value * (index + 1) | 0) | 0,
    _minimum: smaller(ledger._minimum, value),
    _maximum: larger(ledger._maximum, value),
  };
}

function finishLedger(ledger) {
  return ledger._total + ledger._weighted + ledger._maximum - ledger._minimum | 0;
}

function analyzeLedger() {
  const count = algorithmCount();
  let ledger = beginLedger(algorithmInt(0));
  for (let index = 1; index < count; index++) {
    ledger = appendLedger(ledger, algorithmInt(index), index);
  }
  return finishLedger(ledger);
}

console.log(analyzeLedger());
