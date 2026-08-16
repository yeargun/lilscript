export function performanceNow(): number {
  return performance.now()
}

export function scheduleAnimationFrame(callback: FrameRequestCallback): number {
  if (typeof requestAnimationFrame !== "undefined") {
    return requestAnimationFrame(callback)
  }
  return 0
}

export function scheduleMicrotask(callback: () => void): void {
  queueMicrotask(callback)
}

export function hasOwn(obj: object, key: string): boolean {
  return key in obj
}

export function isEventTarget(value: unknown): boolean {
  return typeof EventTarget !== "undefined" && value instanceof EventTarget
}

export function isNodeList(value: unknown): boolean {
  return typeof NodeList !== "undefined" && value instanceof NodeList
}

export function querySelectorAll(
  root: ParentNode,
  selector: string
): Element[] {
  return Array.from(root.querySelectorAll(selector))
}

export function documentQuerySelectorAll(selector: string): Element[] {
  return Array.from(document.querySelectorAll(selector))
}

export function arrayFromNullable(list: ArrayLike<any> | null | undefined): any[] {
  if (list == null) return []
  return Array.from(list).filter((element) => element != null)
}

export function supportsLinearEasingCheck(): boolean {
  try {
    document
      .createElement("div")
      .animate({ opacity: 0 }, { easing: "linear(0, 1)" })
  } catch (e) {
    return false
  }
  return true
}

export function supportsPartialKeyframesCheck(): boolean {
  try {
    document.createElement("div").animate({ opacity: [1] } as any)
  } catch (e) {
    return false
  }
  return true
}

export function supportsWaapiCheck(): boolean {
  return typeof Element !== "undefined" && "animate" in Element.prototype
}

export function documentCreateElement(tag: string): HTMLElement {
  return document.createElement(tag)
}

export function documentHead(): HTMLHeadElement {
  return document.head
}

export function documentGetAnimations(): any[] {
  if (typeof (document as any).getAnimations !== "function") return []
  return Array.from((document as any).getAnimations())
}

export function documentStartViewTransition(callback: () => void): any {
  return (document as any).startViewTransition(callback)
}

export function asMap(value: any): Map<any, any> | null {
  return value instanceof Map ? value : null
}

export function asSet(value: any): Set<any> | null {
  return value instanceof Set ? value : null
}

export function asArray(value: any): any[] | null {
  return Array.isArray(value) ? value : null
}

export function hasScrollTimeline(): boolean {
  return typeof (window as any).ScrollTimeline !== "undefined"
}

export function hasViewTimeline(): boolean {
  return typeof (window as any).ViewTimeline !== "undefined"
}

export function windowInnerWidth(): number {
  return window.innerWidth
}

export function windowInnerHeight(): number {
  return window.innerHeight
}

export function addWindowListener(
  eventName: string,
  handler: EventListenerOrEventListenerObject,
  options?: boolean | AddEventListenerOptions
): void {
  window.addEventListener(eventName, handler, options)
}

export function removeWindowListener(
  eventName: string,
  handler: EventListenerOrEventListenerObject,
  options?: boolean | EventListenerOptions
): void {
  window.removeEventListener(eventName, handler, options)
}

export function addEventListener(
  target: EventTarget,
  eventName: string,
  handler: EventListenerOrEventListenerObject,
  options?: boolean | AddEventListenerOptions
): void {
  target.addEventListener(eventName, handler, options)
}

export function removeEventListener(
  target: EventTarget,
  eventName: string,
  handler: EventListenerOrEventListenerObject,
  options?: boolean | EventListenerOptions
): void {
  target.removeEventListener(eventName, handler, options)
}

export function createResizeObserver(
  callback: ResizeObserverCallback
): ResizeObserver | null {
  if (typeof ResizeObserver === "undefined") return null
  return new ResizeObserver(callback)
}

export function observeResize(observer: ResizeObserver, element: Element): void {
  observer.observe(element)
}

export function unobserveResize(observer: ResizeObserver, element: Element): void {
  observer.unobserve(element)
}

export function readBorderBoxSize(
  borderBoxSize: ReadonlyArray<ResizeObserverSize> | undefined,
  axis: "inlineSize" | "blockSize"
): number | undefined {
  if (borderBoxSize && borderBoxSize[0]) {
    return borderBoxSize[0][axis]
  }
  return undefined
}

export function readElementSize(
  target: Element,
  svgAxis: "width" | "height",
  htmlAxis: "offsetWidth" | "offsetHeight"
): number {
  if ("ownerSVGElement" in target && "getBBox" in (target as any)) {
    return (target as SVGGraphicsElement).getBBox()[svgAxis]
  }
  return (target as any)[htmlAxis]
}

export function getElementTagName(element: Element): string {
  return element.tagName
}

export function constructConfigured(
  Ctor: any,
  arg: any
): any {
  return new Ctor(arg)
}

export function constructConfigured2(
  Ctor: any,
  arg1: any,
  arg2: any
): any {
  return new Ctor(arg1, arg2)
}

export function parseFloatValue(v: any): number {
  return parseFloat(v)
}

export function hasFunction(obj: any, name: string): boolean {
  return Boolean(obj && typeof obj[name] === "function")
}

export function callMethod0(obj: any, name: string): any {
  return obj[name]()
}

export function callMethod1(obj: any, name: string, arg: any): any {
  return obj[name](arg)
}

export function callMethod2(obj: any, name: string, a: any, b: any): any {
  return obj[name](a, b)
}

export function createPointerEvent(type: string, init: any): PointerEvent {
  return new PointerEvent(type, init)
}

export function dispatchEvent(target: EventTarget, event: Event): boolean {
  return target.dispatchEvent(event)
}

export function isFunction(v: any): boolean {
  return typeof v === "function"
}

export function invoke1(fn: any, a: any): void {
  fn(a)
}

export function invoke2(fn: any, a: any, b: any): void {
  fn(a, b)
}

export function callFn3(fn: any, a: any, b: any, c: any): any {
  return fn(a, b, c)
}

export function callFn0(fn: any): any {
  return fn()
}

export function callFn1(fn: any, a: any): any {
  return fn(a)
}

export function callFn2(fn: any, a: any, b: any): any {
  return fn(a, b)
}

export function callFn5(fn: any, a: any, b: any, c: any, d: any, e: any): any {
  return fn(a, b, c, d, e)
}

export function taskFromCallback(runner: (resolve: () => void) => void): Promise<boolean> {
  return new Promise((resolve) => {
    runner(() => resolve(true))
  })
}


export function getComputedStyleValue(element: Element): CSSStyleDeclaration {
  return window.getComputedStyle(element)
}

export function getComputedStyleProperty(element: Element, name: string): string {
  return window.getComputedStyle(element).getPropertyValue(name)
}

export function getComputedStyleField(element: Element, name: string): any {
  const style = window.getComputedStyle(element) as any
  return style[name]
}

export function windowScrollTo(x: number, y: number): void {
  window.scrollTo(x, y)
}

export function windowPageYOffset(): number {
  return window.pageYOffset
}

export function windowGet(name: string): any {
  return (window as any)[name]
}

export function windowSelf(): any {
  return window
}

export function documentElement(): HTMLElement {
  return document.documentElement
}

export function matchMediaQuery(query: string): MediaQueryList | null {
  if (typeof window === "undefined" || !window.matchMedia) return null
  return window.matchMedia(query)
}

export function hasWindow(): boolean {
  return typeof window !== "undefined"
}

export function addMatchMediaListener(query: string, callback: () => void): (() => void) | null {
  if (typeof window === "undefined" || !window.matchMedia) return null
  const mql = window.matchMedia(query)
  const handler = () => callback()
  mql.addEventListener("change", handler)
  callback()
  return () => mql.removeEventListener("change", handler)
}

export function matchMediaMatches(query: string): boolean {
  if (typeof window === "undefined" || !window.matchMedia) return false
  return window.matchMedia(query).matches
}

export function documentGet(name: string): any {
  return (document as any)[name]
}

export function documentScrollingElement(): Element | null {
  return document.scrollingElement
}

export function setProp(obj: any, key: string, value: any): void {
  obj[key] = value
}

export function getProp(obj: any, key: string): any {
  return obj?.[key]
}

export function deleteProp(obj: any, key: string): void {
  if (obj && typeof obj === "object") delete obj[key]
}

export function mapKeys(map: Map<any, any>): any[] {
  return Array.from(map.keys())
}

export function mapValues(map: Map<any, any>): any[] {
  return Array.from(map.values())
}

export function setValues(set: Set<any>): any[] {
  return Array.from(set.values())
}
