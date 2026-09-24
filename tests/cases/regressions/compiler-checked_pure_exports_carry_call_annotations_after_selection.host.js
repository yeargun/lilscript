// Host of compiler.rs checked_pure_exports_carry_call_annotations_after_selection:
// the probe was `process.stdout.write(String(m.callback()))`.
{
  globalThis.check = callback => { console.log(String(callback())); };
}
