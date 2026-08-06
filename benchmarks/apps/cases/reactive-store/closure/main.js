class IntSignal {
  constructor(value) {
    this.value = value;
    this.observers = [];
  }

  read() {
    return this.value;
  }

  watch(observer) {
    this.observers.push(observer);
  }

  write(next) {
    if (next === this.value) return;
    this.value = next;
    for (let index = 0; index < this.observers.length; index += 1) {
      this.observers[index](next);
    }
  }
}

let digest = 0;
const quantity = new IntSignal(3);
const price = new IntSignal(19);
const discount = new IntSignal(2);

function updateTotal() {
  const total = (quantity.read() * price.read() | 0) - discount.read();
  digest = ((digest * 33 | 0) + total) | 0;
  return digest;
}

function unusedForecast(months) {
  return (months * 8_191 | 0) + 17;
}

quantity.watch(updateTotal);
price.watch(updateTotal);
discount.watch(updateTotal);
updateTotal();
for (let index = 0; index < 150_000; index += 1) {
  quantity.write((index % 23) + 1);
  price.write((index % 41) + 3);
  if (index % 4 === 0) discount.write(index % 11);
}
console.log(`reactive:${digest}:${(quantity.read() * price.read() | 0) - discount.read()}`);
