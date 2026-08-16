const sale = /sale/i;
const fresh = /new/i;
const featured = /featured/i;
const limited = /limited offer/i;
let hits = 0;
for (let index = 0; index < 20000; index = (index + 1) | 0) {
  if (sale.test("SUMMER SALE")) hits = (hits + 1) | 0;
  if (fresh.test("brand new")) hits = (hits + 1) | 0;
  if (featured.test("ordinary")) hits = (hits + 1) | 0;
  if (limited.test("limited offer")) hits = (hits + 1) | 0;
}
console.log(hits);
