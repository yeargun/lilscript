export function observe(api) {
  return {exports:Object.keys(api), values:[-2147483648,-1,0,1,2147483647].map(api.transform)};
}
