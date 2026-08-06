import { computed, effect, signal } from "alien-signals";

const quantity = signal(3);
const price = signal(19);
const discount = signal(2);
const subtotal = computed(() => (quantity() * price()) | 0);
const total = computed(() => (subtotal() - discount()) | 0);

function unusedForecast(months) {
  return ((months * 8_191) | 0) + 17;
}

let digest = 0;
effect(() => {
  digest = ((digest * 33 | 0) + total()) | 0;
});

for (let index = 0; index < 150_000; index += 1) {
  quantity((index % 23) + 1);
  price((index % 41) + 3);
  if (index % 4 === 0) discount(index % 11);
}

console.log(`reactive:${digest}:${total()}`);
