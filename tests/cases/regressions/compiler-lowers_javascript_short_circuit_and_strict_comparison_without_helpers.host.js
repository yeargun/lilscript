// Host of compiler.rs lowers_javascript_short_circuit_and_strict_comparison_without_helpers
// (runner.mjs). `check` replays the test's runner against the handed-over functions.
{
  const calls = [];
  globalThis.left = () => { calls.push("l"); return 0; };
  globalThis.right = () => { calls.push("r"); return 7; };
  globalThis.check = (fallback, guarded, same, different) => {
    console.log(fallback(), calls.join(""));
    calls.length = 0;
    console.log(guarded(), calls.join(""));
    console.log(same(0), same("0"), different(false), different(0));
  };
}
