// Host of compiler.rs private_global_store_survives_cross_function_conditional_selection
// (flag=true): `function flag(){return true}`.
{
  globalThis.flag = function flag() { return true; };
}
