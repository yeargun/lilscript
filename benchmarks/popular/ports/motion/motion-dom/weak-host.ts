export function weakMapCreate(): WeakMap<any, any> {
  return new WeakMap()
}

export function weakMapGet(map: WeakMap<any, any>, key: any): any {
  const value = map.get(key)
  return value === undefined ? null : value
}

export function weakMapSet(map: WeakMap<any, any>, key: any, value: any): void {
  map.set(key, value)
}

export function weakMapHas(map: WeakMap<any, any>, key: any): boolean {
  return map.has(key)
}

export function weakMapDelete(map: WeakMap<any, any>, key: any): void {
  map.delete(key)
}

export function weakSetCreate(): WeakSet<any> {
  return new WeakSet()
}

export function weakSetAdd(set: WeakSet<any>, value: any): void {
  set.add(value)
}

export function weakSetHas(set: WeakSet<any>, value: any): boolean {
  return set.has(value)
}

export function weakSetDelete(set: WeakSet<any>, value: any): void {
  set.delete(value)
}
