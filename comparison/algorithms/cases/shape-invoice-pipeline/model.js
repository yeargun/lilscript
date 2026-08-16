function clampDiscount(discount) {
  return discount < 0 ? 0 : discount > 75 ? 75 : discount;
}

export function makeLine(unit, quantity, discount, tax) {
  return { _unit: unit, _quantity: quantity, _discount: clampDiscount(discount), _tax: tax };
}

function discounted(line) {
  const gross = line._unit * line._quantity | 0;
  return gross - ((gross * line._discount | 0) / 100 | 0) | 0;
}

function taxed(line) {
  const net = discounted(line);
  return net + ((net * line._tax | 0) / 100 | 0) | 0;
}

function lineTotal(line) {
  return taxed(line) + (line._quantity * 3 | 0) | 0;
}

export function beginSummary() {
  return { _total: 0, _tax: 0, _count: 0, _fingerprint: 0 };
}

export function appendSummary(summary, line) {
  const net = discounted(line);
  const withTax = taxed(line);
  return {
    _total: summary._total + lineTotal(line) | 0,
    _tax: summary._tax + withTax - net | 0,
    _count: summary._count + line._quantity | 0,
    _fingerprint: summary._fingerprint ^ withTax + (line._unit * 17 | 0),
  };
}

function checksumSummary(summary) {
  return summary._fingerprint ^ (summary._tax * 11 | 0) + (summary._count * 7 | 0);
}

export function finishSummary(summary) {
  return summary._total + checksumSummary(summary) | 0;
}
