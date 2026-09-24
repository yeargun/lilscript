// Host of compiler.rs size_first_search_spreads_a_delimiter_not_the_live_match:
// `let seen=[];function consume(n){seen.push(n)};var cap=['full',null,null,'**',null,null,null,null]`,
// and after the program `process.stdout.write(seen.join(','))`.
{
  const seen = [];
  globalThis.consume = n => { seen.push(n); };
  globalThis.cap = ['full', null, null, '**', null, null, null, null];
  queueMicrotask(() => console.log(seen.join(',')));
}
