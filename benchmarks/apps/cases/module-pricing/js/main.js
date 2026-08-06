import "./startup.js";
import { Quote } from "./model.js";
import { basketTotal } from "./pricing.js";

const quotes = [new Quote(3, 199), new Quote(5, 349), new Quote(2, 1_299)];
let digest = 0;
for (let index = 0; index < 120_000; index += 1) {
  digest =
    (((digest + basketTotal(quotes)) | 0) + (index % 17) | 0) % 1_000_000_007;
}
console.log(`modules:${digest}:${basketTotal(quotes)}`);
