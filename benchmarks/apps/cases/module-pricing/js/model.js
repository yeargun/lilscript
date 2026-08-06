export class Quote {
  constructor(units, cents) {
    this.units = units;
    this.cents = cents;
  }
}

export function lineTotal(quote) {
  return (quote.units * quote.cents) | 0;
}

export function unusedQuoteScore(quote) {
  return ((lineTotal(quote) * 65_537) | 0) + 91;
}
