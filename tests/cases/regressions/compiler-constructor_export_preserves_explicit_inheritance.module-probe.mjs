// Probe of compiler.rs constructor_export_preserves_explicit_inheritance. The test appended
// `let value=new Child(3,4);process.stdout.write([Child.name,Child.length,value instanceof Base,
// value instanceof Child,value.read(),value.total()].join(':'))` to the module text. `Base` is not
// exported, so the probe reaches it as the constructor Child extends (Object.getPrototypeOf(Child)).
export default function probe(m) {
  const Child = m.Child;
  const Base = Object.getPrototypeOf(Child);
  let value = new Child(3, 4);
  console.log([Child.name, Child.length, value instanceof Base, value instanceof Child, value.read(), value.total()].join(':'));
}
