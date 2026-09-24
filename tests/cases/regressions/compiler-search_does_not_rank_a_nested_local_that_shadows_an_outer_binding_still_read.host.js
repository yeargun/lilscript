// Host of compiler.rs search_does_not_rank_a_nested_local_that_shadows_an_outer_binding_still_read:
// `let go,seen=[];function keep(cb){go=cb}function observe(v){seen.push(typeof v)}`, and after the
// program `go(function(){seen.push('called')});process.stdout.write(seen.join(','))`.
{
  let go;
  const seen = [];
  globalThis.keep = cb => { go = cb; };
  globalThis.observe = v => { seen.push(typeof v); };
  queueMicrotask(() => {
    go(function () { seen.push('called'); });
    console.log(seen.join(','));
  });
}
