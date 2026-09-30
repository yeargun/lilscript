const canonical="0189dcd5-5311-7d40-8db0-9496a2eef37b";
const inputs=[canonical,"0189dcd553117d408db09496a2eef37b",`{${canonical}}`,`urn:uuid:${canonical}`];
export function run(api,iterations) {
  let characters=0;
  for(let index=0;index<iterations;index++) for(const input of inputs) {
    const output=api.parseUuid(input);
    if(output!==canonical) throw new Error("UUID normalization oracle differs");
    characters+=output.length;
  }
  return {oracle:{characters,canonical},counters:{parse_calls:iterations*4}};
}
