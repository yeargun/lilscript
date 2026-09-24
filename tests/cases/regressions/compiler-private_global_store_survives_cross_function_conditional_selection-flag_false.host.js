// Host of compiler.rs private_global_store_survives_cross_function_conditional_selection
// (flag=false): `function flag(){return false}`.
{
  globalThis.flag = function flag() { return false; };
}
