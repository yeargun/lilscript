export class Quote {
  constructor(units, cents) {
    this.units = units;
    this.cents = cents;
  }
}

export function lineTotal(quote) {
  return Math.imul(quote.units, quote.cents);
}

export function unusedQuoteScore(quote) {
  return Math.imul(lineTotal(quote), 65_537) + 91;
}
