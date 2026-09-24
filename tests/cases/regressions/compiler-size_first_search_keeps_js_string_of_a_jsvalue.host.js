// Host of compiler.rs size_first_search_keeps_js_string_of_a_jsvalue:
// `let seen=[];function consume(a,b){seen.push(typeof a,String(a),typeof b)};
//  var file={toString(){return 'hello'}}`, and after the program `process.stdout.write(seen.join(':'))`.
{
  const seen = [];
  globalThis.consume = (a, b) => { seen.push(typeof a, String(a), typeof b); };
  globalThis.file = { toString() { return 'hello'; } };
  queueMicrotask(() => console.log(seen.join(':')));
}
