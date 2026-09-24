// Probe of compiler.rs explicit_constructor_export_preserves_named_class_identity. The test appended
// `let value=new Scale(3);process.stdout.write([Scale.name,Scale.length,value.factor,value.apply(4)].join(':'))`
// to the module text; `Scale` is the binding published as `PublicScale`.
export default function probe(m) {
  const Scale = m.PublicScale;
  let value = new Scale(3);
  console.log([Scale.name, Scale.length, value.factor, value.apply(4)].join(':'));
}
