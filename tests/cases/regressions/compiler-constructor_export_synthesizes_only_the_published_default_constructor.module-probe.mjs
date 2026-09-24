// Probe of compiler.rs constructor_export_synthesizes_only_the_published_default_constructor. The test
// appended `let value=new Empty;process.stdout.write([Empty.name,Empty.length,value.value,value.ready,
// value.label,value.items.length,value.ping()].join(':'))` to the module text.
export default function probe(m) {
  const Empty = m.Empty;
  let value = new Empty;
  console.log([Empty.name, Empty.length, value.value, value.ready, value.label, value.items.length, value.ping()].join(':'));
}
