import { lineTotal } from "./model.js";

export function basketTotal(quotes) {
  let total = 0;
  for (let index = 0; index < quotes.length; index += 1) {
    total = (total + lineTotal(quotes[index])) | 0;
  }
  return total;
}

export function unusedRegionalPrice(quotes) {
  return (basketTotal(quotes) + 999_983) | 0;
}
