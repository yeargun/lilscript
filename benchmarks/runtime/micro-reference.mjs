export function calls(value) {
  let result=value;
  for(let index=0;index<32;index++) result=((result+index)|0)^(index<<2);
  return result;
}
export function allocation(value) {
  const values=[value,(value+1)|0,(value+2)|0,(value+3)|0];
  values.push((value+4)|0);
  let result=0;for(const item of values) result=(result+item)|0;
  return result;
}
export function strings(value) { const names=["alpha","beta","gamma","delta"];return names[value&3].length; }
