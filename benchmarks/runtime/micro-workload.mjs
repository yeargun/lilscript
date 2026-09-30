// Independent closed-form checks for allocation/string results. The call
// recurrence has a separately expressed reference in the declared oracle.
export function run(api,iterations) {
  let allocations=0,lengths=0,calls=0;
  for(let index=0;index<iterations;index++) {
    allocations+=api.allocation(index&255);
    lengths+=api.strings(index);
    calls+=api.calls(index&255);
  }
  return {oracle:{allocations,lengths,calls},counters:{calls:iterations*3,iterations}};
}
