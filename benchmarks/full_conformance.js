(() => {
class Counter {
  constructor(initial) {
    this.value = initial;
  }

  add(amount) {
    this.value = (this.value + amount) | 0;
    return this.value;
  }
}

function square(value) {
  return Math.imul(value, value);
}

const pair = { left: 2, right: 5 };
const counter = new Counter((pair.left + pair.right) | 0);
const classValue = counter.add(3);

const factor = 2;
const scale = (value) => Math.imul(value, factor);
const values = [1, 2, 3, 4];
const mapped = values.map(scale);
const selected = mapped.filter((value) => value >= 4);
const total = selected.reduce((sum, value) => (sum + value) | 0, 0);

let whileTotal = 0;
let whileIndex = 0;
while (whileIndex < 3) {
  whileTotal = (whileTotal + square(whileIndex)) | 0;
  whileIndex = (whileIndex + 1) | 0;
}

let flow = 0;
for (let index = 0; index < 8; index = (index + 1) | 0) {
  if (index === 2) continue;
  if (index === 6) break;
  flow = (flow + index) | 0;
}

let arithmetic = 20;
arithmetic = (arithmetic - 2) | 0;
arithmetic = Math.imul(arithmetic, 3);
arithmetic = (arithmetic / 2) | 0;
arithmetic = (arithmetic % 10) | 0;
const logic = (total === 18 && flow === 13) || false;

const label = "LilScript";
const message = label.toLowerCase() + ":" + classValue;
const checks = label.includes("Script") && label.startsWith("Lil") && label.endsWith("Script");

console.log(`values=${total},while=${whileTotal},flow=${flow},math=${arithmetic}`);
console.log(`logic=${logic},checks=${checks},message=${message}`);
})();
