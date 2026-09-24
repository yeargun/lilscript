// The module's exports are its observations: run() prints, shown() computes.
export default function probe(m) {
  m.run();
  console.log(m.shown(4));
}
