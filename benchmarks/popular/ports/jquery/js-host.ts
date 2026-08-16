export function windowSelf(): any {
  return typeof window !== "undefined" ? window : globalThis
}

export function windowDocument(): any {
  const w = windowSelf()
  return w.document
}

export function documentElementOf(doc: any): any {
  return doc.documentElement
}

export function objectToStringTag(value: any): string {
  return Object.prototype.toString.call(value)
}

export function functionToString(value: any): string {
  return Function.prototype.toString.call(value)
}

export function objectHasOwn(obj: any, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(obj, key)
}

export function getPrototypeOf(value: any): any {
  return Object.getPrototypeOf(value)
}

export function typeOf(value: any): string {
  return typeof value
}

export function isNullish(value: any): boolean {
  return value == null
}

export function isFalse(value: any): boolean {
  return value === false
}

export function isUndefined(value: any): boolean {
  return value === undefined
}

export function stringify(value: any): string {
  return value + ""
}

// Compiler-recognized explicit invariant boundary. This fallback is an
// identity for foreign toolchains; optimized LilScript JavaScript erases it.
export function jsAssume<T>(value: any): T {
  return value as T
}

export function createEmptyObject(): any {
  return {}
}

export function createNullProtoObject(): any {
  return Object.create(null)
}

export function createArray(): any[] {
  return []
}

export function arrayPush(arr: any, item: any): number {
  return Array.prototype.push.call(arr, item)
}

export function arrayPop(arr: any): any {
  return Array.prototype.pop.call(arr)
}

export function arraySlice(arr: any, start?: number, end?: number): any[] {
  if (arguments.length <= 1) return Array.prototype.slice.call(arr)
  if (arguments.length === 2) return Array.prototype.slice.call(arr, start)
  return Array.prototype.slice.call(arr, start, end)
}

export function arrayIndexOf(arr: any, elem: any, fromIndex?: number): number {
  if (fromIndex === undefined) return Array.prototype.indexOf.call(arr, elem)
  return Array.prototype.indexOf.call(arr, elem, fromIndex)
}

export function arraySort(arr: any, compare?: any): any {
  if (compare === undefined || compare === null) return Array.prototype.sort.call(arr)
  return Array.prototype.sort.call(arr, compare)
}

export function arraySplice(arr: any, start: number, deleteCount?: number, item?: any): any[] {
  if (arguments.length === 2) return Array.prototype.splice.call(arr, start)
  if (arguments.length === 3 || item === undefined || item === null) {
    return Array.prototype.splice.call(arr, start, deleteCount as number)
  }
  return Array.prototype.splice.call(arr, start, deleteCount as number, item)
}

export function arrayConcatApply(target: any, arrays: any): any[] {
  return Array.prototype.concat.apply(target, arrays)
}

export function arrayFlat(array: any): any[] {
  if (typeof Array.prototype.flat === "function") {
    return Array.prototype.flat.call(array)
  }
  return Array.prototype.concat.apply([], array)
}

export function objectConstructor(): any {
  return Object
}

export function arrayIsArray(value: any): boolean {
  return Array.isArray(value)
}

export function call0(fn: any, thisArg: any): any {
  return fn.call(thisArg)
}

export function call1(fn: any, thisArg: any, a: any): any {
  return fn.call(thisArg, a)
}

export function call2(fn: any, thisArg: any, a: any, b: any): any {
  return fn.call(thisArg, a, b)
}

export function call3(fn: any, thisArg: any, a: any, b: any, c: any): any {
  return fn.call(thisArg, a, b, c)
}

export function apply(fn: any, thisArg: any, args: any): any {
  return fn.apply(thisArg, args)
}

export function isFunctionValue(obj: any): boolean {
  return typeof obj === "function" && typeof obj.nodeType !== "number" && typeof obj.item !== "function"
}

export function isWindowValue(obj: any): boolean {
  return obj != null && obj === obj.window
}

export function objectKeys(obj: any): string[] {
  return Object.keys(obj)
}

export function mathRandom(): number {
  return Math.random()
}

export function stringReplace(
  s: string,
  pattern: RegExp | string,
  replacement: string | ((substring: string, ...args: any[]) => string)
): string {
  return s.replace(pattern as any, replacement as any)
}

export function stringReplace2(
  s: string,
  pattern: RegExp,
  replacement: (substring: string, a: string) => string
): string {
  return s.replace(pattern, replacement as any)
}

export function stringMatch(s: string, pattern: RegExp): any {
  return s.match(pattern)
}

export function stringSplit(s: string, sep: string | RegExp): string[] {
  return s.split(sep as any)
}

export function arrayJoin(arr: any, sep: string): string {
  return Array.prototype.join.call(arr, sep)
}

export function regexTest(re: RegExp, s: string): boolean {
  return re.test(s)
}

export function regexExec(re: RegExp, s: string): RegExpExecArray | null {
  return re.exec(s)
}

export function setProp(obj: any, key: string, value: any): void {
  obj[key] = value
}

export function getProp(obj: any, key: string): any {
  return obj[key]
}

export function deleteProp(obj: any, key: string): void {
  delete obj[key]
}

export function objectBox(value: any): any {
  return Object(value)
}

export function hasProp(obj: any, key: string): boolean {
  return obj != null && key in Object(obj)
}

export function defineIterator(obj: any, iterator: any): void {
  if (typeof Symbol === "function") {
    obj[Symbol.iterator] = iterator
  }
}

export function getArrayIterator(): any {
  return [][Symbol.iterator as any]
}

export function noop(): void {}

export function throwError(msg: string): never {
  throw new Error(msg)
}

export function throwTypeError(msg: string): never {
  throw new TypeError(msg)
}

export function jsUndefined(): any {
  return undefined
}

export const UNDEFINED: any = undefined

export function scheduleTimeout(fn: any): void {
  const w = windowSelf()
  w.setTimeout(fn, 0)
}

export function call4(fn: any, thisArg: any, a: any, b: any, c: any, d: any): any {
  return fn.call(thisArg, a, b, c, d)
}

export function consoleWarn3(a: any, b: any, c: any): void {
  const w = windowSelf()
  if (w.console && typeof w.console.warn === "function") {
    w.console.warn(a, b, c)
  }
}

export function nodeNameOf(elem: any): any {
  return elem && elem.nodeName
}

export function getNodeType(elem: any): any {
  return elem && elem.nodeType
}

export function getTextContent(elem: any): any {
  return elem.textContent
}

export function getNodeValue(elem: any): any {
  return elem.nodeValue
}

export function getNamespaceURI(elem: any): any {
  return elem && elem.namespaceURI
}

export function getOwnerDocument(elem: any): any {
  return elem && elem.ownerDocument
}

export function objectCreate(proto: any): any {
  return Object.create(proto)
}

export function setLength(obj: any, length: number): void {
  obj.length = length
}

export function newRegexp(pattern: string, flags?: string): RegExp {
  return flags === undefined ? new RegExp(pattern) : new RegExp(pattern, flags)
}

export function defineConfigurable(obj: any, key: string, value: any): void {
  Object.defineProperty(obj, key, {
    value,
    configurable: true,
  })
}

export function unaryPlus(value: any): number {
  return +value
}

export function getAttribute(elem: any, name: string): any {
  return elem.getAttribute(name)
}

export function setAttribute(elem: any, name: string, value: string): void {
  elem.setAttribute(name, value)
}

export function removeAttribute(elem: any, name: string): void {
  elem.removeAttribute(name)
}

export function createElement(doc: any, tag: string): any {
  return doc.createElement(tag)
}

export function appendChild(parent: any, child: any): any {
  return parent.appendChild(child)
}

export function matchesSelector(elem: any, selector: string): boolean {
  const m = elem.matches || elem.webkitMatchesSelector || elem.msMatchesSelector
  return m ? m.call(elem, selector) : false
}

export function dateNow(): number {
  return Date.now()
}

export function parseIntRadix(value: string, radix: number): number {
  return parseInt(value, radix)
}

export function stringReplaceFirst(s: string, search: string, replacement: string): string {
  return s.replace(search, replacement)
}

export function addEventListener(target: any, eventName: string, handler: any, capture?: boolean): void {
  target.addEventListener(eventName, handler, capture === true)
}

export function removeEventListener(target: any, eventName: string, handler: any, capture?: boolean): void {
  target.removeEventListener(eventName, handler, capture === true)
}

export function debugLog(value: any): void {
  console.log("DEBUG:", value)
}

export function scheduleTimeoutMs(fn: any, ms: number): any {
  return windowSelf().setTimeout(fn, ms)
}

export function clearTimeoutId(id: any): void {
  windowSelf().clearTimeout(id)
}

export function throwValue(error: any): never {
  throw error
}

export function arrayShift(arr: any): any {
  return Array.prototype.shift.call(arr)
}

export function arrayUnshift(arr: any, item: any): number {
  return Array.prototype.unshift.call(arr, item)
}

export function stringSlice(s: string, start: number, end?: number): string {
  if (end === undefined) return s.slice(start)
  return s.slice(start, end)
}

export function stringIndexOf(s: string, search: string, fromIndex?: number): number {
  if (fromIndex === undefined) return s.indexOf(search)
  return s.indexOf(search, fromIndex)
}

export function newDOMParser(): any {
  return new DOMParser()
}

export function parseHexEscape(hexDigits: string): number {
  return Number("0x" + hexDigits) - 0x10000
}

export function stringFromCharCode1(a: number): string {
  return String.fromCharCode(a)
}

export function stringFromCharCode2(a: number, b: number): string {
  return String.fromCharCode(a, b)
}

export function mathRound(value: number): number {
  return Math.round(value)
}

export function mathCeil(value: number): number {
  return Math.ceil(value)
}

export function mathMax(a: number, b: number): number {
  return Math.max(a, b)
}

export function mathMin(a: number, b: number): number {
  return Math.min(a, b)
}

export function mathCos(value: number): number {
  return Math.cos(value)
}

export function mathPI(): number {
  return Math.PI
}

export function requestAnimationFrameOrNull(fn: any): any {
  const w = windowSelf()
  if (typeof w.requestAnimationFrame === "function") {
    return w.requestAnimationFrame(fn)
  }
  return undefined
}

export function parseFloatValue(value: string): number {
  return parseFloat(value)
}

export function isFiniteValue(value: number): boolean {
  return isFinite(value)
}

export function encodeURIComponentValue(value: string): string {
  return encodeURIComponent(value)
}

export function newXMLHttpRequest(): any {
  try {
    return new (windowSelf() as any).XMLHttpRequest()
  } catch (e) {
    return undefined
  }
}
