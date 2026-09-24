// Host of compiler.rs preserves_effectful_calls_before_conditional_returns:
// `let callback;function retain(value){callback=value}`, and after the program
// `console.log(callback());console.log(callback())`.
{
  let callback;
  globalThis.retain = function retain(value) { callback = value; };
  queueMicrotask(() => { console.log(callback()); console.log(callback()); });
}
