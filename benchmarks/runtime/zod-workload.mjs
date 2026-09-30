export function run(api,iterations) {
  const z=api.z;
  const schema=z.object({id:z.number().int(),name:z.string().min(1),tags:z.array(z.string()),ok:z.boolean()});
  let sum=0,failures=0;
  const output=[];
  for(let index=0;index<iterations;index++) {
    const input={id:index&255,name:"user",tags:["one","two"],ok:true};
    const result=schema.parse(input);
    if(result===input || result.tags===input.tags || result.name!=="user" || result.tags.join(",")!=="one,two" || !result.ok) throw new Error("schema copy/value oracle differs");
    sum+=result.id;
    failures+=Number(!schema.safeParse({...input,id:"wrong"}).success);
    if(index<8) output.push(result);
  }
  return {oracle:{sum,failures,retained_rows:output.length},counters:{parse_calls:iterations,safe_parse_calls:iterations},retained:output};
}
