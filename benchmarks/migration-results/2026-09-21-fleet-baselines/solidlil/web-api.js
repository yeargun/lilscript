import {
  ChildProperties,
  DOMElements,
  DOMWithState,
  DelegatedEvents,
  MathMLElements,
  Namespaces,
  RawTextElements,
  SVGElements,
  VoidElements,
} from "./dom-tables.js"
import {
  Errored,
  For,
  Hydration,
  Loading,
  Match,
  NoHydration,
  Repeat,
  Reveal,
  Show,
  Switch,
  children,
  createComponent,
  createMemo,
  createRenderEffect,
  createRoot,
  enableHydration,
  finishHydration,
  flush,
  getOwner,
  merge,
  onCleanup,
  sharedConfig,
  untrack,
} from "./solid-api.js"
import * as lil from "./lil-web.js"

export {
  ChildProperties,
  DOMElements,
  DOMWithState,
  DelegatedEvents,
  Errored,
  For,
  Hydration,
  Loading,
  Match,
  MathMLElements,
  Namespaces,
  NoHydration,
  RawTextElements,
  Repeat,
  Reveal,
  SVGElements,
  Show,
  Switch,
  VoidElements,
  children,
  createComponent,
  getOwner,
  untrack,
}

export const HREF = Symbol.for("solid.Href")
export const SAFE_ERROR = Symbol.for("solid.SafeError")
export const RequestContext = Symbol.for("solid.RequestContext")
export const REVALIDATE_HEADER = "X-Revalidate"
export const isServer = false
export const isDev = false

const delegatedRoots = new Set()
const delegatedContainers = new Map()
const elementClaims = new Set()
const hydrationEvents = []
let requestEvent
let hydrateCursor = null
const assets = new Map()
const headTags = []

export function render(code, element, init, options) {
  if (typeof code === "string") return lil.render(code, element)
  const hydrating = options?.hydrate || sharedConfig.hydrating
  return createRoot((dispose) => {
    if (!hydrating && element && "textContent" in element) element.textContent = ""
    insert(element, code, undefined, init)
    flush()
    onCleanup(() => {
      if (element && "textContent" in element) element.textContent = ""
    })
    return dispose
  })
}

export function hydrate(code, node, options) {
  enableHydration()
  hydrateCursor = node?.firstChild ?? null
  const dispose = render(code, node, undefined, { ...options, hydrate: true })
  finishHydration()
  hydrateCursor = null
  runHydrationEvents()
  return dispose
}

export function insert(parent, accessor, marker, current) {
  if (typeof accessor !== "function") {
    return applyInsert(parent, accessor, marker, current)
  }
  let nodes = current
  createRenderEffect(() => {
    nodes = applyInsert(parent, accessor(), marker, nodes)
  })
  return nodes
}

function isDomNode(value) {
  return Boolean(
    value &&
      typeof value === "object" &&
      (typeof value.nodeType === "number" ||
        (typeof Node !== "undefined" && value instanceof Node)),
  )
}

function asNodes(value) {
  if (value == null || value === false || value === true) return []
  if (typeof value === "function") return asNodes(value())
  if (typeof value === "string" || typeof value === "number") {
    if (typeof document === "undefined") return []
    return [document.createTextNode(String(value))]
  }
  if (Array.isArray(value)) return value.flatMap(asNodes)
  if (isDomNode(value)) return [value]
  return asNodes(value)
}

function applyInsert(parent, value, marker, previous) {
  const next = asNodes(value)
  const prev = Array.isArray(previous) ? previous : previous ? [previous] : []
  for (const node of prev) {
    if (!next.includes(node) && node.parentNode) node.parentNode.removeChild(node)
  }
  let cursor = marker ?? null
  for (const node of next) {
    if (sharedConfig.hydrating && node.parentNode === parent) continue
    if (node !== cursor && typeof parent.insertBefore === "function") {
      parent.insertBefore(node, cursor)
    }
    cursor = node.nextSibling ?? cursor
  }
  return next
}

export function template(html, flag) {
  const node = document.createElement("template")
  node.innerHTML = html
  const first = () => {
    const content = flag === 2 ? node.content.firstChild : node.content
    const root = content.firstChild ?? content
    return root.cloneNode(true)
  }
  return () => {
    if (sharedConfig.hydrating) return getNextElement(first)
    return first()
  }
}

export function getHydrationKey() {
  return sharedConfig.getNextContextId?.()
}

export function getNextElement(factory) {
  if (sharedConfig.hydrating && hydrateCursor) {
    const node = hydrateCursor
    hydrateCursor = hydrateCursor.nextSibling
    return node
  }
  return typeof factory === "function" ? factory() : document.createElement("div")
}

export function getNextMatch(start, elementName) {
  let cursor = start
  while (cursor) {
    if (cursor.nodeName?.toLowerCase() === elementName.toLowerCase()) return cursor
    cursor = cursor.nextSibling
  }
  return getNextElement()
}

export function getNextMarker(start) {
  const nodes = []
  let cursor = start
  while (cursor) {
    if (cursor.nodeType === 8) return [cursor, nodes]
    nodes.push(cursor)
    cursor = cursor.nextSibling
  }
  return [start, nodes]
}

export function runHydrationEvents() {
  while (hydrationEvents.length) hydrationEvents.shift()?.()
}

export function Portal(props) {
  const mount = props.mount ?? (typeof document === "undefined" ? null : document.body)
  const nodes = children(() => props.children)
  if (mount) {
    createRenderEffect(() => {
      const list = nodes.toArray()
      for (const node of list) mount.appendChild(node)
    })
    onCleanup(() => {
      for (const node of nodes.toArray()) node.parentNode?.removeChild(node)
    })
  }
  return document?.createTextNode("") ?? null
}

export function dynamic(source) {
  return (props) => {
    const Comp = typeof source === "function" ? source() : source
    if (!Comp) return undefined
    if (typeof Comp === "string") {
      const node = document.createElement(Comp)
      assign(node, props)
      return node
    }
    return createComponent(Comp, props)
  }
}

export function Dynamic(props) {
  const { component, ...rest } = props
  return dynamic(() => component)(rest)
}

export function clientOnly(fn, options) {
  let loaded
  const Comp = (props) => {
    if (sharedConfig.hydrating) return props.fallback
    if (!loaded) {
      if (!options?.lazy) fn().then((mod) => { loaded = mod.default })
      return props.fallback
    }
    return loaded(props)
  }
  if (!options?.lazy) fn().then((mod) => { loaded = mod.default })
  return Comp
}

export function httpStatus() {}
export function httpHeader() {}

export function effect(fn, apply) {
  return createRenderEffect(fn, apply)
}

export function memo(fn) {
  return createMemo(fn)
}

export function scope(fn) {
  return fn
}

export function mergeProps(...sources) {
  return merge(...sources)
}

export function spread(node, accessor, skipChildren) {
  const apply = (props) => assign(node, props, skipChildren)
  if (typeof accessor === "function") createRenderEffect(() => apply(accessor()))
  else apply(accessor)
}

export function assign(node, props, skipChildren, prev = {}, skipRef) {
  const next = props ?? {}
  for (const key of new Set([...Object.keys(prev), ...Object.keys(next)])) {
    if (key === "children" && skipChildren) continue
    if (key === "ref" && skipRef) continue
    if (key === "class" || key === "className") className(node, next[key], prev[key])
    else if (key === "style") style(node, next[key], prev[key])
    else if (key.startsWith("on")) addEvent(node, key.slice(2).toLowerCase(), next[key], DelegatedEvents.has(key.slice(2).toLowerCase()))
    else if (next[key] == null) node.removeAttribute?.(key)
    else if (key in node && !ChildProperties.has(key)) node[key] = next[key]
    else setAttribute(node, key, next[key])
  }
  return next
}

export function setAttribute(node, name, value) {
  if (value == null) node.removeAttribute(name)
  else node.setAttribute(name, String(value))
}

export function setAttributeNS(node, namespace, name, value) {
  if (value == null) node.removeAttributeNS(namespace, name)
  else node.setAttributeNS(namespace, name, String(value))
}

export function className(node, value) {
  node.className = value == null ? "" : String(value)
}

export function setProperty(node, name, value) {
  node[name] = value
}

export function setStyleProperty(node, name, value) {
  node.style[name] = value
}

export function style(node, value, prev = {}) {
  const next = value ?? {}
  for (const key of new Set([...Object.keys(prev), ...Object.keys(next)])) {
    node.style[key] = next[key] ?? ""
  }
}

export function addEvent(node, name, handler, delegate) {
  if (!handler) return
  if (delegate) {
    node[`$$${name}`] = handler
    return
  }
  node.addEventListener(name, handler)
}

export function delegateEvents(eventNames) {
  for (const name of eventNames) {
    document.addEventListener(name, (event) => {
      let target = event.target
      while (target) {
        const handler = target[`$$${name}`]
        if (handler) {
          handler(event)
          return
        }
        target = target.parentNode
      }
    })
  }
}

export function registerDelegatedRoot(root) {
  delegatedRoots.add(root)
}
export function unregisterDelegatedRoot(root) {
  delegatedRoots.delete(root)
}
export function registerDelegatedContainer(container, owner) {
  delegatedContainers.set(container, owner)
}
export function unregisterDelegatedContainer(container) {
  delegatedContainers.delete(container)
}
export function getDelegatedRoot(node) {
  let cursor = node
  while (cursor) {
    if (delegatedRoots.has(cursor)) return cursor
    cursor = cursor.parentNode
  }
}

export function registerElementClaim(handler) {
  elementClaims.add(handler)
  return () => elementClaims.delete(handler)
}
export function claimElement(node) {
  for (const handler of elementClaims) handler(node)
  return node
}
export function claimElementTree(root) {
  root.querySelectorAll?.("a[href],form[action]")?.forEach(claimElement)
  return root
}

export function dynamicProperty(props, key) {
  return () => props[key]
}

export function applyRef(ref, element) {
  const list = Array.isArray(ref) ? ref : [ref]
  for (const fn of list) if (typeof fn === "function") fn(element)
}

export function ref(fn, element) {
  applyRef(fn(), element)
}

export function useHead(tag) {
  const tags = typeof tag === "function" ? tag() : tag
  headTags.push(tags)
}

export function acquireAsset(descriptor) {
  assets.set(descriptor, (assets.get(descriptor) ?? 0) + 1)
  return () => {
    const count = (assets.get(descriptor) ?? 1) - 1
    if (count <= 0) assets.delete(descriptor)
    else assets.set(descriptor, count)
  }
}

export function warmAsset(descriptor) {
  if (descriptor?.type === "style" || descriptor?.type === "module") {
    return { loadState: "loaded", loadPromise: Promise.resolve() }
  }
}

export function HydrationScript() {
  return null
}

export function generateHydrationScript() {
  return `<script>window._$HY=window._$HY||{events:[],completed:new WeakSet};</script>`
}

export function getRequestEvent() {
  return requestEvent
}

export function createRequestEvent(request = new Request("http://localhost"), locals = {}) {
  requestEvent = { request, locals, response: createResponseStub() }
  return requestEvent
}

export function createResponseStub() {
  return { headers: new Headers(), committed: false }
}

export function commitResponseStub(stub) {
  if (stub) stub.committed = true
}

export function commitEventResponse(event) {
  commitResponseStub(event?.response)
}

export function createSSRResponse(body, init) {
  return new Response(body, init)
}

export function createLiveHoles() {
  return []
}

export function composeMiddleware(...handlers) {
  return (event) => handlers.reduce((next, handler) => () => handler(event, next), () => {})()
}

export function parseCookieHeader(header) {
  const map = new Map()
  if (!header) return map
  for (const part of String(header).split(";")) {
    const [name, ...rest] = part.trim().split("=")
    if (!name) continue
    const value = rest.join("=")
    try {
      map.set(decodeURIComponent(name), decodeURIComponent(value.replace(/^"|"$/g, "")))
    } catch {
      map.set(name, value)
    }
  }
  return map
}

export function serializeCookie(name, value, options = {}) {
  const parts = [`${encodeURIComponent(name)}=${encodeURIComponent(value)}`]
  parts.push(`Path=${options.path ?? "/"}`)
  if (options.domain) parts.push(`Domain=${options.domain}`)
  if (options.maxAge != null) parts.push(`Max-Age=${Math.trunc(options.maxAge)}`)
  if (options.expires) parts.push(`Expires=${options.expires.toUTCString()}`)
  if (options.httpOnly) parts.push("HttpOnly")
  if (options.secure) parts.push("Secure")
  if (options.sameSite) parts.push(`SameSite=${options.sameSite}`)
  return parts.join("; ")
}

export function hasFlashCookie() {
  return false
}
export function clearFlashCookie() {}
export function isServerFunction() {
  return false
}
export function getServerFunctionMetadata() {}
export function getServerFunctionRPC() {}

export function isHref() {
  return false
}
export function isResponseEnvelope(value) {
  return value && value[ResponseEnvelope]
}
export const ResponseEnvelope = Symbol("solid.ResponseEnvelope")
export function isSafeError(error) {
  return Boolean(error?.[SAFE_ERROR])
}
export function markSafeError(error) {
  if (error && typeof error === "object") error[SAFE_ERROR] = true
  return error
}
export function getExpectedRedirectStatus() {
  return 302
}
export function redirect(url, status = 302) {
  const response = { url, status, [ResponseEnvelope]: true }
  return response
}
export function reload() {
  if (typeof location !== "undefined") location.reload()
}
export function respond(body, init) {
  return new Response(body, init)
}

export function escape(text) {
  return String(text)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;")
}

function stringifyNode(value) {
  if (value == null || value === false || value === true) return ""
  if (typeof value === "function") return stringifyNode(value())
  if (typeof value === "string" || typeof value === "number") return escape(value)
  if (Array.isArray(value)) return value.map(stringifyNode).join("")
  if (typeof document !== "undefined" && value instanceof Node) {
    const host = document.createElement("div")
    host.appendChild(value.cloneNode(true))
    return host.innerHTML
  }
  return String(value ?? "")
}

export function renderToString(code) {
  if (typeof document !== "undefined") {
    const host = document.createElement("div")
    const dispose = render(code, host)
    const html = host.innerHTML
    dispose()
    return html
  }
  return stringifyNode(typeof code === "function" ? code() : code)
}

export function renderToStream(code) {
  const html = renderToString(code)
  return new ReadableStream({
    start(controller) {
      controller.enqueue(html)
      controller.close()
    },
  })
}

export function ssr(template, ...values) {
  if (typeof template === "string") return template
  return template.reduce((html, part, index) => html + part + (values[index] ?? ""), "")
}

export function ssrElement(tag, props, children, selfClose) {
  const attributes = typeof props === "function" ? props() : props ?? {}
  const attr = Object.entries(attributes)
    .filter(([, value]) => value != null && value !== false)
    .map(([name, value]) => ` ${name}="${escape(value === true ? "" : value)}"`)
    .join("")
  const inner = typeof children === "function" ? children() : children ?? ""
  if (selfClose || VoidElements.has(tag)) return `<${tag}${attr}>`
  return `<${tag}${attr}>${inner}</${tag}>`
}

export function ssrAttribute(name, value) {
  if (value == null || value === false) return ""
  return ` ${name}="${escape(value === true ? "" : value)}"`
}

export function ssrClassName(value) {
  return value ? ` class="${escape(value)}"` : ""
}

export function ssrStyle(value) {
  if (!value) return ""
  const text = typeof value === "string"
    ? value
    : Object.entries(value).filter(([, next]) => next != null).map(([name, next]) => `${name}:${next}`).join(";")
  return text ? ` style="${escape(text)}"` : ""
}

export function ssrStyleProperty(name, value) {
  return value == null ? "" : `${name}:${value};`
}

export function ssrHydrationKey() {
  return ` data-hk="${getHydrationKey()}"`
}

export function ssrGroup(value) {
  return stringifyNode(value)
}

void lil
