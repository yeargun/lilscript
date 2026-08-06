class Quote {
  constructor(units, cents) {
    this.units = units;
    this.cents = cents;
  }
}

function lineTotal(quote) {
  return (quote.units * quote.cents) | 0;
}

function basketTotal(quotes) {
  let total = 0;
  for (let index = 0; index < quotes.length; index += 1) {
    total = (total + lineTotal(quotes[index])) | 0;
  }
  return total;
}

function unusedRegionalPrice(quotes) {
  return (basketTotal(quotes) + 999_983) | 0;
}

console.log("module:init");
const quotes = [new Quote(3, 199), new Quote(5, 349), new Quote(2, 1_299)];
let digest = 0;
for (let index = 0; index < 120_000; index += 1) {
  digest =
    (((digest + basketTotal(quotes)) | 0) + (index % 17) | 0) % 1_000_000_007;
}
console.log(`modules:${digest}:${basketTotal(quotes)}`);
