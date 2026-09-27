// Probe of tests/cases/nominal/export_constructor_boundary.lil.
export default function probe(m) {
  const { Counter, Tagged } = m;
  const counter = new Counter(3);
  const made = m.make(1);
  const tagged = new Tagged("t", 4);
  const created = m.tag("u");
  let throws = false;
  try { Counter(1); } catch (error) { throws = error instanceof TypeError; }
  console.log([
    Counter.name, Counter.length, Tagged.name, Tagged.length,
    counter.next(), counter.next(2), counter.peek(),
    Counter.prototype.next.length, Counter.prototype.peek.length,
    Object.keys(Counter.prototype).length, Object.keys(counter).join(","),
    made instanceof Counter, made.next(),
    tagged instanceof Tagged, tagged instanceof Counter, tagged.show(), tagged.next(),
    created instanceof Tagged, created.show(),
    Object.getPrototypeOf(Tagged) === Counter, throws,
  ].join(":"));
}
