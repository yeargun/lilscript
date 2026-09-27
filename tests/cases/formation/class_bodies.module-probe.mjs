// Probe of tests/cases/formation/class_bodies.lil: the classes' shape at the
// ES module boundary is JavaScript's: an implicit constructor where the class
// has none, `length`, non-enumerable prototype methods, `new` required.
export default function probe(m) {
  const { Greeter, Counter, Doubler } = m;
  const greeter = new Greeter();
  const counter = new Counter(1);
  const doubler = new Doubler(2);
  let throws = false;
  try { Greeter(); } catch (error) { throws = error instanceof TypeError; }
  console.log([
    Greeter.name, Greeter.length, Counter.length, Doubler.length,
    Object.keys(Greeter.prototype).length, Object.keys(Counter.prototype).length,
    Greeter.prototype.hello.length, Counter.prototype.bump.length, Counter.prototype.reader.length,
    greeter.hello("x"), greeter.twice("y"), counter.bump(2), counter.reader()(),
    Greeter.prototype.hello.call({}, "z"), doubler.value, doubler instanceof Counter,
    Object.getPrototypeOf(Doubler) === Counter, throws,
  ].join(":"));
}
