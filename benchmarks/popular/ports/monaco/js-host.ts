export function stringSlice(s: string, start: number, end?: number): string {
  if (end === undefined) return s.slice(start)
  return s.slice(start, end)
}

export function stringReplace(s: string, pattern: RegExp, replacement: string): string {
  return s.replace(pattern, replacement)
}

export function stringSplit(s: string, sep: string): string[] {
  return s.split(sep)
}

export function stringFromCharCode1(a: number): string {
  return String.fromCharCode(a)
}

export function newRegexp(pattern: string, flags = ""): RegExp {
  return new RegExp(pattern, flags)
}

export function regexTest(re: RegExp, s: string): boolean {
  return re.test(s)
}

export function regexExec(re: RegExp, s: string): RegExpExecArray | null {
  return re.exec(s)
}

export function regexLastIndex(re: RegExp): number {
  return re.lastIndex
}

export function regexSetLastIndex(re: RegExp, value: number): void {
  re.lastIndex = value
}

export function dateNow(): number {
  return Date.now()
}

export function mathRandom(): number {
  return Math.random()
}

export function scheduleTimeoutMs(fn: () => void, ms: number): any {
  return setTimeout(fn, ms)
}

export function clearTimeoutId(id: any): void {
  clearTimeout(id)
}

export function requestAnimationFrameOrNull(fn: (t: number) => void): any {
  const raf = (globalThis as any).requestAnimationFrame
  if (typeof raf === "function") return raf.call(globalThis, fn)
  return setTimeout(() => fn(Date.now()), 16)
}

export function createElement(tag: string): HTMLElement {
  return document.createElement(tag)
}

export function domCreateElement(tag: string): HTMLElement {
  return document.createElement(tag)
}

export function createTextNode(text: string): Text {
  return document.createTextNode(text)
}

export function appendChild(parent: Node, child: Node): Node {
  return parent.appendChild(child)
}

export function removeChild(parent: Node, child: Node): Node {
  return parent.removeChild(child)
}

export function setTextContent(el: Node, text: string): void {
  el.textContent = text
}

export function getTextContent(el: Node): string {
  return el.textContent ?? ""
}

export function setInnerHTML(el: Element, html: string): void {
  el.innerHTML = html
}

export function getInnerHTML(el: Element): string {
  return el.innerHTML
}

export function setClassName(el: Element, name: string): void {
  el.className = name
}

export function setStyle(el: HTMLElement, prop: string, value: string): void {
  el.style.setProperty(prop, value)
}

export function setAttribute(el: Element, name: string, value: string): void {
  el.setAttribute(name, value)
}

export function addEventListener(target: EventTarget, eventName: string, handler: any, capture = false): void {
  target.addEventListener(eventName, handler, capture)
}

export function removeEventListener(target: EventTarget, eventName: string, handler: any, capture = false): void {
  target.removeEventListener(eventName, handler, capture)
}

export function focusElement(el: HTMLElement): void {
  el.focus()
}

export function getClientWidth(el: HTMLElement): number {
  return el.clientWidth | 0
}

export function getClientHeight(el: HTMLElement): number {
  return el.clientHeight | 0
}

export function getScrollTop(el: HTMLElement): number {
  return el.scrollTop | 0
}

export function setScrollTop(el: HTMLElement, value: number): void {
  el.scrollTop = value
}

export function getScrollHeight(el: HTMLElement): number {
  return el.scrollHeight | 0
}

export function preventDefault(event: Event): void {
  event.preventDefault()
}

export function stopPropagation(event: Event): void {
  event.stopPropagation()
}

export function eventKey(event: KeyboardEvent): string {
  return event.key
}

export function eventCode(event: KeyboardEvent): string {
  return event.code
}

export function eventKeyCode(event: KeyboardEvent): number {
  return event.keyCode | 0
}

export function eventCtrlKey(event: KeyboardEvent): boolean {
  return event.ctrlKey || event.metaKey
}

export function eventShiftKey(event: KeyboardEvent): boolean {
  return event.shiftKey
}

export function eventAltKey(event: KeyboardEvent): boolean {
  return event.altKey
}

export function eventInputValue(event: Event): string {
  const t = event.target as HTMLTextAreaElement | HTMLInputElement | null
  return t && typeof t.value === "string" ? t.value : ""
}

export function inputSetValue(el: HTMLTextAreaElement | HTMLInputElement, value: string): void {
  el.value = value
}

export function inputGetValue(el: HTMLTextAreaElement | HTMLInputElement): string {
  return el.value
}

export function canvasGetContext2d(canvas: HTMLCanvasElement): CanvasRenderingContext2D | null {
  try {
    return canvas.getContext("2d")
  } catch {
    return null
  }
}

export function canvasSetSize(canvas: HTMLCanvasElement, width: number, height: number): void {
  canvas.width = width
  canvas.height = height
}

export function canvasFillRect(ctx: CanvasRenderingContext2D | null, x: number, y: number, w: number, h: number, color: string): void {
  if (!ctx) {
    return
  }
  ctx.fillStyle = color
  ctx.fillRect(x, y, w, h)
}

export function clearChildren(el: Node): void {
  while (el.firstChild) {
    el.removeChild(el.firstChild)
  }
}

export function setTabIndex(el: HTMLElement, value: number): void {
  el.tabIndex = value
}

export function getClipboardText(): Promise<string> {
  if (navigator.clipboard && navigator.clipboard.readText) {
    return navigator.clipboard.readText()
  }
  return Promise.resolve("")
}

export function setClipboardText(text: string): Promise<void> {
  memoryClipboard = text
  if (navigator.clipboard && navigator.clipboard.writeText) {
    return navigator.clipboard.writeText(text)
  }
  return Promise.resolve()
}

let memoryClipboard = ""

export function clipboardRead(): string {
  return memoryClipboard
}

export function clipboardWrite(text: string): void {
  memoryClipboard = text
  try {
    void navigator.clipboard?.writeText(text)
  } catch {
  }
}

export function clipboardReadEvent(event: ClipboardEvent): string {
  const data = event.clipboardData?.getData("text/plain")
  if (typeof data === "string" && data.length > 0) {
    memoryClipboard = data
    return data
  }
  return memoryClipboard
}

export function clipboardWriteEvent(event: ClipboardEvent, text: string): void {
  memoryClipboard = text
  event.clipboardData?.setData("text/plain", text)
}

export function eventClientX(event: MouseEvent): number {
  return event.clientX | 0
}

export function eventClientY(event: MouseEvent): number {
  return event.clientY | 0
}

export function eventButton(event: MouseEvent): number {
  return event.button | 0
}

export function eventDetail(event: MouseEvent): number {
  return event.detail | 0
}

export function rectLeft(el: Element): number {
  return el.getBoundingClientRect().left | 0
}

export function rectTop(el: Element): number {
  return el.getBoundingClientRect().top | 0
}

export function getScrollLeft(el: HTMLElement): number {
  return el.scrollLeft | 0
}

export function setScrollLeft(el: HTMLElement, value: number): void {
  el.scrollLeft = value
}

export function blurElement(el: HTMLElement): void {
  el.blur()
}

export function setPlaceholder(el: HTMLInputElement | HTMLTextAreaElement, value: string): void {
  el.placeholder = value
}

export function setDisplay(el: HTMLElement, value: string): void {
  el.style.display = value
}

export function classListAdd(el: Element, name: string): void {
  el.classList.add(name)
}

export function classListRemove(el: Element, name: string): void {
  el.classList.remove(name)
}

export function hostCall(obj: any, name: string, a?: any, b?: any, c?: any): any {
  if (obj == null) {
    return undefined
  }
  const fn = obj[name]
  if (typeof fn !== "function") {
    return undefined
  }
  try {
    return fn.call(obj, a, b, c)
  } catch (err) {
    console.warn("hostCall", name, err)
    return undefined
  }
}

export function hostThen(value: any, cb: (v: any) => void): void {
  if (value && typeof value.then === "function") {
    value.then(cb, () => cb(undefined))
    return
  }
  cb(value)
}

export function jsArrayLen(value: any): number {
  if (Array.isArray(value)) {
    return value.length
  }
  if (value && Array.isArray(value.suggestions)) {
    return value.suggestions.length
  }
  if (value && Array.isArray(value.items)) {
    return value.items.length
  }
  if (value && Array.isArray(value.lenses)) {
    return value.lenses.length
  }
  if (value && Array.isArray(value.symbols)) {
    return value.symbols.length
  }
  if (value && Array.isArray(value.contents)) {
    return value.contents.length
  }
  return 0
}

export function jsArrayAt(value: any, index: number): any {
  if (Array.isArray(value)) {
    return value[index]
  }
  const arr = value?.suggestions ?? value?.items ?? value?.lenses ?? value?.symbols ?? value?.contents
  return Array.isArray(arr) ? arr[index] : undefined
}

export function jsPropString(value: any, key: string, fallback = ""): string {
  if (value == null) {
    return fallback
  }
  const v = value[key]
  if (v == null) {
    return fallback
  }
  if (typeof v === "string") {
    return v
  }
  if (typeof v === "object" && typeof v.value === "string") {
    return v.value
  }
  if (typeof v === "object" && typeof v.label === "string") {
    return v.label
  }
  return String(v)
}

export function jsPropInt(value: any, key: string, fallback = 0): number {
  if (value == null) {
    return fallback
  }
  const v = value[key]
  return typeof v === "number" ? (v | 0) : fallback
}

export function emptyBuf(): string {
  return Array(1).join("")
}

export function beginIdList(holder: any): void {
  holder._ids = []
}

export function pushId(holder: any, id: number): void {
  holder._ids.push(id | 0)
}

export function takeIdList(holder: any): number[] {
  const out = holder._ids ?? []
  holder._ids = []
  return out
}

export function concat2(a: string, b: string): string {
  return a + b
}

export function pushNewBuffer(tree: any, text: string, lineStarts: number[]): void {
  const buf = { buffer: text, lineStarts }
  tree.tmpBuffer = buf
  tree.buffers.push(buf)
}

export function makeNodePos(node: any, remainder: number, start: number) {
  const rem = remainder | 0
  const off = start | 0
  const pos: any = [node, rem, off]
  pos.node = node
  pos.remainder = rem
  pos.nodeStartOffset = off
  return pos
}

function hostNodeNext(node: any, sentinel: any): any {
  if (node.right !== sentinel) {
    let cur = node.right
    while (cur.left !== sentinel) {
      cur = cur.left
    }
    return cur
  }
  let cur = node
  let guard = 0
  while (cur.parent !== sentinel && guard < 100000) {
    guard++
    if (cur.parent.left === cur) {
      break
    }
    cur = cur.parent
  }
  if (cur.parent === sentinel || guard >= 100000) {
    return sentinel
  }
  return cur.parent
}

function hostAccumulatedValue(tree: any, node: any, index: number): number {
  if (index < 0) {
    return 0
  }
  const piece = node.piece
  const lineStarts = tree.buffers[piece.bufferIndex].lineStarts
  const expected = (piece.start.line + index + 1) | 0
  if (expected > piece.end.line) {
    return ((lineStarts[piece.end.line] | 0) + piece.end.column - (lineStarts[piece.start.line] | 0) - piece.start.column) | 0
  }
  return ((lineStarts[expected] | 0) - (lineStarts[piece.start.line] | 0) - piece.start.column) | 0
}

function hostOffsetOfNode(tree: any, node: any): number {
  let pos = node.size_left | 0
  let cur = node
  while (cur !== tree.root) {
    if (cur.parent.right === cur) {
      pos = (pos + (cur.parent.size_left | 0) + cur.parent.piece.length) | 0
    }
    cur = cur.parent
  }
  return pos
}

function hostNodeAt(tree: any, offset: number, sentinel: any) {
  offset = offset | 0
  if (tree.cacheValid) {
    const cached = tree.cacheNode
    const cStart = tree.cacheNodeStartOffset | 0
    const cLen = cached.piece.length | 0
    if (cStart <= offset && cStart + cLen >= offset) {
      return makeNodePos(cached, offset - cStart, cStart)
    }
  }
  let rest = offset
  let x = tree.root
  let nodeStartOffset = 0
  let guard = 0
  while (x !== sentinel && guard < 100000) {
    const sizeLeft = x.size_left | 0
    const pieceLen = x.piece.length | 0
    if (sizeLeft > rest) {
      x = x.left
    } else if (sizeLeft + pieceLen >= rest) {
      nodeStartOffset = (nodeStartOffset + sizeLeft) | 0
      tree.cacheNode = x
      tree.cacheNodeStartOffset = nodeStartOffset
      tree.cacheNodeStartLineNumber = 0
      tree.cacheHasLine = false
      tree.cacheValid = true
      return makeNodePos(x, rest - sizeLeft, nodeStartOffset)
    } else {
      rest = (rest - sizeLeft - pieceLen) | 0
      nodeStartOffset = (nodeStartOffset + sizeLeft + pieceLen) | 0
      x = x.right
    }
    guard++
  }
  return makeNodePos(sentinel, 0, nodeStartOffset)
}

function hostNodeAt2(tree: any, lineNumber: number, column: number, sentinel: any) {
  let line = lineNumber | 0
  let col = column | 0
  let x = tree.root
  let nodeStartOffset = 0
  let guard = 0
  while (x !== sentinel && guard < 100000) {
    const lfLeft = x.lf_left | 0
    const pieceLf = x.piece.lineFeedCnt | 0
    const xLeft = x.left
    if (xLeft !== sentinel && lfLeft >= line - 1) {
      x = xLeft
    } else if (lfLeft + pieceLf > line - 1) {
      const prevAccumulatedValue = hostAccumulatedValue(tree, x, line - lfLeft - 2)
      const accumulatedValue = hostAccumulatedValue(tree, x, line - lfLeft - 1)
      nodeStartOffset = (nodeStartOffset + x.size_left) | 0
      let rem = (prevAccumulatedValue + col - 1) | 0
      if (rem > accumulatedValue) {
        rem = accumulatedValue
      }
      return makeNodePos(x, rem, nodeStartOffset)
    } else if (lfLeft + pieceLf === line - 1) {
      const prevAccumulatedValue = hostAccumulatedValue(tree, x, line - lfLeft - 2)
      const pieceLen = x.piece.length | 0
      if ((prevAccumulatedValue + col - 1) <= pieceLen) {
        return makeNodePos(x, (prevAccumulatedValue + col - 1) | 0, nodeStartOffset)
      }
      col = (col - (pieceLen - prevAccumulatedValue)) | 0
      break
    } else {
      line = (line - lfLeft - pieceLf) | 0
      nodeStartOffset = (nodeStartOffset + x.size_left + x.piece.length) | 0
      x = x.right
    }
    guard++
  }
  x = hostNodeNext(x, sentinel)
  let walk = 0
  while (x !== sentinel && walk < 100000) {
    const pieceLf = x.piece.lineFeedCnt | 0
    const pieceLen = x.piece.length | 0
    if (pieceLf > 0) {
      const accumulatedValue = hostAccumulatedValue(tree, x, 0)
      const start = hostOffsetOfNode(tree, x)
      let rem = (col - 1) | 0
      if (rem > accumulatedValue) {
        rem = accumulatedValue
      }
      return makeNodePos(x, rem, start)
    } else if (pieceLen >= col - 1) {
      return makeNodePos(x, (col - 1) | 0, hostOffsetOfNode(tree, x))
    } else {
      col = (col - pieceLen) | 0
    }
    x = hostNodeNext(x, sentinel)
    walk++
  }
  return makeNodePos(sentinel, 0, nodeStartOffset)
}

export function nodeAtStash(tree: any, offset: number, sentinel: any, slot: number): void {
  if (!tree._stash) {
    tree._stash = [null, null]
  }
  tree._stash[slot | 0] = hostNodeAt(tree, offset, sentinel)
}

export function nodeAt2Stash(tree: any, lineNumber: number, column: number, sentinel: any, slot: number): void {
  if (!tree._stash) {
    tree._stash = [null, null]
  }
  tree._stash[slot | 0] = hostNodeAt2(tree, lineNumber, column, sentinel)
}

export function stashNode(tree: any, slot: number): any {
  return tree._stash[slot | 0].node
}

export function stashRem(tree: any, slot: number): number {
  return tree._stash[slot | 0].remainder | 0
}

export function stashStart(tree: any, slot: number): number {
  return tree._stash[slot | 0].nodeStartOffset | 0
}

function hostCursorLine(cursor: any): number {
  return (cursor.line ?? cursor[0] ?? 0) | 0
}

function hostCursorCol(cursor: any): number {
  return (cursor.column ?? cursor[1] ?? 0) | 0
}

function hostOffsetInBuffer(tree: any, bufferIndex: number, cursor: any): number {
  const lineStarts = tree.buffers[bufferIndex].lineStarts
  return (lineStarts[hostCursorLine(cursor)] + hostCursorCol(cursor)) | 0
}

function hostGetLineFeedCnt(tree: any, bufferIndex: number, start: any, end: any): number {
  const endLine = hostCursorLine(end)
  const endCol = hostCursorCol(end)
  const startLine = hostCursorLine(start)
  if (endCol === 0) {
    return (endLine - startLine) | 0
  }
  const lineStarts = tree.buffers[bufferIndex].lineStarts
  if (endLine === lineStarts.length - 1) {
    return (endLine - startLine) | 0
  }
  const nextLineStartOffset = lineStarts[endLine + 1] | 0
  const endOffset = (lineStarts[endLine] + endCol) | 0
  if (nextLineStartOffset > endOffset + 1) {
    return (endLine - startLine) | 0
  }
  const previousCharOffset = (endOffset - 1) | 0
  if (tree.buffers[bufferIndex].buffer.charCodeAt(previousCharOffset) === 13) {
    return (endLine - startLine + 1) | 0
  }
  return (endLine - startLine) | 0
}

function hostPositionInBuffer(tree: any, node: any, remainder: number) {
  const piece = node.piece
  const lineStarts = tree.buffers[piece.bufferIndex].lineStarts
  const startOffset = (lineStarts[piece.start.line] + piece.start.column + (remainder | 0)) | 0
  let low = piece.start.line | 0
  let high = piece.end.line | 0
  let mid = low
  let midStart = 0
  let midStop = 0
  while (low <= high) {
    mid = (low + (((high - low) / 2) | 0)) | 0
    midStart = lineStarts[mid] | 0
    if (mid === lineStarts.length - 1) {
      midStop = tree.buffers[piece.bufferIndex].buffer.length | 0
    } else {
      midStop = lineStarts[mid + 1] | 0
    }
    if (startOffset < midStart) {
      high = (mid - 1) | 0
    } else if (startOffset >= midStop) {
      low = (mid + 1) | 0
    } else {
      break
    }
  }
  return { line: mid, column: (startOffset - midStart) | 0 }
}

function hostGetIndexOf(tree: any, node: any, accumulatedValue: number) {
  const piece = node.piece
  const pos = hostPositionInBuffer(tree, node, accumulatedValue)
  const lineCnt = (pos.line - piece.start.line) | 0
  const span =
    (hostOffsetInBuffer(tree, piece.bufferIndex, piece.end) -
      hostOffsetInBuffer(tree, piece.bufferIndex, piece.start)) |
    0
  if (span === (accumulatedValue | 0)) {
    const realLineCnt = hostGetLineFeedCnt(tree, node.piece.bufferIndex, piece.start, pos)
    if (realLineCnt !== lineCnt) {
      return { index: realLineCnt, remainder: 0 }
    }
  }
  return { index: lineCnt, remainder: pos.column | 0 }
}

export function hostGetOffsetAt(tree: any, lineNumber: number, column: number, sentinel: any): number {
  let leftLen = 0
  let x = tree.root
  let line = lineNumber | 0
  let guard = 0
  while (x !== sentinel && guard < 100000) {
    guard++
    if (x.left !== sentinel && (x.lf_left + 1) >= line) {
      x = x.left
    } else if ((x.lf_left + x.piece.lineFeedCnt + 1) >= line) {
      leftLen = (leftLen + x.size_left) | 0
      const acc = hostAccumulatedValue(tree, x, (line - x.lf_left - 2) | 0)
      return (leftLen + acc + (column | 0) - 1) | 0
    } else {
      line = (line - x.lf_left - x.piece.lineFeedCnt) | 0
      leftLen = (leftLen + x.size_left + x.piece.length) | 0
      x = x.right
    }
  }
  return leftLen
}

function makePos(line: number, column: number) {
  const lineN = line | 0
  const colN = column | 0
  const pos: any = [lineN, colN]
  pos.lineNumber = lineN
  pos.column = colN
  return pos
}

export function hostGetPositionAt(tree: any, offset: number, sentinel: any) {
  let rest = offset | 0
  if (rest < 0) {
    rest = 0
  }
  let x = tree.root
  let lfCnt = 0
  const originalOffset = rest
  let guard = 0
  while (x !== sentinel && guard < 100000) {
    guard++
    if (x.size_left !== 0 && x.size_left >= rest) {
      x = x.left
    } else if ((x.size_left + x.piece.length) >= rest) {
      const out = hostGetIndexOf(tree, x, (rest - x.size_left) | 0)
      lfCnt = (lfCnt + x.lf_left + out.index) | 0
      if (out.index === 0) {
        const lineStartOffset = hostGetOffsetAt(tree, lfCnt + 1, 1, sentinel)
        const column = (originalOffset - lineStartOffset) | 0
        return makePos(lfCnt + 1, column + 1)
      }
      return makePos(lfCnt + 1, out.remainder + 1)
    } else {
      rest = (rest - x.size_left - x.piece.length) | 0
      lfCnt = (lfCnt + x.lf_left + x.piece.lineFeedCnt) | 0
      if (x.right === sentinel) {
        const lineStartOffset = hostGetOffsetAt(tree, lfCnt + 1, 1, sentinel)
        const column = (originalOffset - rest - lineStartOffset) | 0
        return makePos(lfCnt + 1, column + 1)
      }
      x = x.right
    }
  }
  return makePos(1, 1)
}

function hostCalcSize(node: any, sentinel: any): number {
  if (node === sentinel) {
    return 0
  }
  return (node.size_left + node.piece.length + hostCalcSize(node.right, sentinel)) | 0
}

function hostCalcLf(node: any, sentinel: any): number {
  if (node === sentinel) {
    return 0
  }
  return (node.lf_left + node.piece.lineFeedCnt + hostCalcLf(node.right, sentinel)) | 0
}

function hostLeftest(node: any, sentinel: any): any {
  while (node.left !== sentinel) {
    node = node.left
  }
  return node
}

function hostLeftRotate(tree: any, x: any, sentinel: any): void {
  const y = x.right
  y.size_left = (y.size_left + x.size_left + x.piece.length) | 0
  y.lf_left = (y.lf_left + x.lf_left + x.piece.lineFeedCnt) | 0
  x.right = y.left
  if (y.left !== sentinel) {
    y.left.parent = x
  }
  y.parent = x.parent
  if (x.parent === sentinel) {
    tree.root = y
  } else if (x.parent.left === x) {
    x.parent.left = y
  } else {
    x.parent.right = y
  }
  y.left = x
  x.parent = y
}

function hostRightRotate(tree: any, y: any, sentinel: any): void {
  const x = y.left
  y.left = x.right
  if (x.right !== sentinel) {
    x.right.parent = y
  }
  x.parent = y.parent
  y.size_left = (y.size_left - (x.size_left + x.piece.length)) | 0
  y.lf_left = (y.lf_left - (x.lf_left + x.piece.lineFeedCnt)) | 0
  if (y.parent === sentinel) {
    tree.root = x
  } else if (y === y.parent.right) {
    y.parent.right = x
  } else {
    y.parent.left = x
  }
  x.right = y
  y.parent = x
}

function hostUpdateMeta(tree: any, x: any, delta: number, lfDelta: number, sentinel: any): void {
  while (x !== tree.root && x !== sentinel) {
    if (x.parent.left === x) {
      x.parent.size_left = (x.parent.size_left + delta) | 0
      x.parent.lf_left = (x.parent.lf_left + lfDelta) | 0
    }
    x = x.parent
  }
}

function hostRecomputeMeta(tree: any, x: any, sentinel: any): void {
  if (x === tree.root) {
    return
  }
  while (x !== tree.root && x === x.parent.right) {
    x = x.parent
  }
  if (x === tree.root) {
    return
  }
  x = x.parent
  const delta = (hostCalcSize(x.left, sentinel) - x.size_left) | 0
  const lfDelta = (hostCalcLf(x.left, sentinel) - x.lf_left) | 0
  x.size_left = (x.size_left + delta) | 0
  x.lf_left = (x.lf_left + lfDelta) | 0
  while (x !== tree.root && (delta !== 0 || lfDelta !== 0)) {
    if (x.parent.left === x) {
      x.parent.size_left = (x.parent.size_left + delta) | 0
      x.parent.lf_left = (x.parent.lf_left + lfDelta) | 0
    }
    x = x.parent
  }
}

function hostDetach(node: any): void {
  node.alive = false
  node.parent = node
  node.left = node
  node.right = node
}

export function rbDeleteTree(tree: any, z: any, sentinel: any): void {
  let y: any
  let x: any
  if (z.left === sentinel) {
    y = z
    x = y.right
  } else if (z.right === sentinel) {
    y = z
    x = y.left
  } else {
    y = hostLeftest(z.right, sentinel)
    x = y.right
  }

  if (y === tree.root) {
    tree.root = x
    x.color = 0
    hostDetach(z)
    sentinel.parent = sentinel
    tree.root.parent = sentinel
    return
  }

  const yWasRed = y.color === 1
  if (y === y.parent.left) {
    y.parent.left = x
  } else {
    y.parent.right = x
  }

  if (y === z) {
    x.parent = y.parent
    hostRecomputeMeta(tree, x, sentinel)
  } else {
    if (y.parent === z) {
      x.parent = y
    } else {
      x.parent = y.parent
    }
    hostRecomputeMeta(tree, x, sentinel)
    y.left = z.left
    y.right = z.right
    y.parent = z.parent
    y.color = z.color
    if (z === tree.root) {
      tree.root = y
    } else if (z === z.parent.left) {
      z.parent.left = y
    } else {
      z.parent.right = y
    }
    if (y.left !== sentinel) {
      y.left.parent = y
    }
    if (y.right !== sentinel) {
      y.right.parent = y
    }
    y.size_left = z.size_left
    y.lf_left = z.lf_left
    hostRecomputeMeta(tree, y, sentinel)
  }

  hostDetach(z)

  if (x.parent.left === x) {
    const newSizeLeft = hostCalcSize(x, sentinel)
    const newLFLeft = hostCalcLf(x, sentinel)
    if (newSizeLeft !== x.parent.size_left || newLFLeft !== x.parent.lf_left) {
      const delta = (newSizeLeft - x.parent.size_left) | 0
      const lfDelta = (newLFLeft - x.parent.lf_left) | 0
      x.parent.size_left = newSizeLeft
      x.parent.lf_left = newLFLeft
      hostUpdateMeta(tree, x.parent, delta, lfDelta, sentinel)
    }
  }
  hostRecomputeMeta(tree, x.parent, sentinel)

  if (yWasRed) {
    sentinel.parent = sentinel
    return
  }

  while (x !== tree.root && x.color === 0) {
    if (x === x.parent.left) {
      let w = x.parent.right
      if (w.color === 1) {
        w.color = 0
        x.parent.color = 1
        hostLeftRotate(tree, x.parent, sentinel)
        w = x.parent.right
      }
      if (w.left.color === 0 && w.right.color === 0) {
        w.color = 1
        x = x.parent
      } else {
        if (w.right.color === 0) {
          w.left.color = 0
          w.color = 1
          hostRightRotate(tree, w, sentinel)
          w = x.parent.right
        }
        w.color = x.parent.color
        x.parent.color = 0
        w.right.color = 0
        hostLeftRotate(tree, x.parent, sentinel)
        x = tree.root
      }
    } else {
      let w = x.parent.left
      if (w.color === 1) {
        w.color = 0
        x.parent.color = 1
        hostRightRotate(tree, x.parent, sentinel)
        w = x.parent.left
      }
      if (w.left.color === 0 && w.right.color === 0) {
        w.color = 1
        x = x.parent
      } else {
        if (w.left.color === 0) {
          w.right.color = 0
          w.color = 1
          hostLeftRotate(tree, w, sentinel)
          w = x.parent.left
        }
        w.color = x.parent.color
        x.parent.color = 0
        w.left.color = 0
        hostRightRotate(tree, x.parent, sentinel)
        x = tree.root
      }
    }
  }
  x.color = 0
  sentinel.parent = sentinel
}

function splitLinesHost(text: string): string[] {
  const lines: string[] = []
  let cur = ""
  for (let i = 0; i < text.length; i++) {
    const ch = text.charAt(i)
    if (ch === "\n") {
      lines.push(cur)
      cur = ""
    } else if (ch !== "\r") {
      cur += ch
    }
  }
  lines.push(cur)
  return lines
}

function makeDiffChange(os: number, oe: number, ms: number, me: number) {
  const change: any = [os, oe, ms, me]
  change.originalStart = os
  change.originalEnd = oe
  change.modifiedStart = ms
  change.modifiedEnd = me
  return change
}

class HostFastInts {
  pos: number[] = [0, 0, 0, 0, 0, 0, 0, 0]
  neg: number[] = [0, 0, 0, 0, 0, 0, 0, 0]

  get(idx: number): number {
    if (idx < 0) {
      const i = -idx - 1
      return i >= this.neg.length ? 0 : this.neg[i]
    }
    return idx >= this.pos.length ? 0 : this.pos[idx]
  }

  set(idx: number, value: number): void {
    if (idx < 0) {
      const i = -idx - 1
      while (this.neg.length <= i) {
        this.neg.push(0)
      }
      this.neg[i] = value
    } else {
      while (this.pos.length <= idx) {
        this.pos.push(0)
      }
      this.pos[idx] = value
    }
  }
}

type HostSnake = { prev: HostSnake | null; x: number; y: number; length: number }

class HostFastPaths {
  pos: Array<HostSnake | null> = []
  neg: Array<HostSnake | null> = []

  get(idx: number): HostSnake | null {
    if (idx < 0) {
      const i = -idx - 1
      return i >= this.neg.length ? null : this.neg[i]
    }
    return idx >= this.pos.length ? null : this.pos[idx]
  }

  set(idx: number, value: HostSnake | null): void {
    if (idx < 0) {
      const i = -idx - 1
      while (this.neg.length <= i) {
        this.neg.push(null)
      }
      this.neg[i] = value
    } else {
      while (this.pos.length <= idx) {
        this.pos.push(null)
      }
      this.pos[idx] = value
    }
  }
}

function hostSnake(seqX: string[], seqY: string[], x: number, y: number): number {
  let xi = x | 0
  let yi = y | 0
  while (xi < seqX.length && yi < seqY.length && seqX[xi] === seqY[yi]) {
    xi++
    yi++
  }
  return xi
}

export function hostComputeLineDiff(original: string, modified: string) {
  const seqX = splitLinesHost(original)
  const seqY = splitLinesHost(modified)
  const result: any[] = []
  if (seqX.length === 0 && seqY.length === 0) {
    return result
  }
  if (seqX.length === 0) {
    result.push(makeDiffChange(0, 0, 0, seqY.length))
    return result
  }
  if (seqY.length === 0) {
    result.push(makeDiffChange(0, seqX.length, 0, 0))
    return result
  }
  const V = new HostFastInts()
  const paths = new HostFastPaths()
  const first = hostSnake(seqX, seqY, 0, 0)
  V.set(0, first)
  paths.set(0, first === 0 ? null : { prev: null, x: 0, y: 0, length: first })
  let foundK = 0
  let done = false
  let d = 0
  const limit = seqX.length + seqY.length + 2
  while (!done && d <= limit) {
    d++
    const lowerBound = -Math.min(d, seqY.length + (d % 2))
    const upperBound = Math.min(d, seqX.length + (d % 2))
    for (let k = lowerBound; k <= upperBound; k += 2) {
      let maxXofDLineTop = -1
      if (k !== upperBound) {
        maxXofDLineTop = V.get(k + 1)
      }
      let maxXofDLineLeft = -1
      if (k !== lowerBound) {
        maxXofDLineLeft = V.get(k - 1) + 1
      }
      const x = Math.min(Math.max(maxXofDLineTop, maxXofDLineLeft), seqX.length)
      const y = x - k
      if (x <= seqX.length && y <= seqY.length) {
        const newMaxX = hostSnake(seqX, seqY, x, y)
        V.set(k, newMaxX)
        const lastPath = x === maxXofDLineTop ? paths.get(k + 1) : paths.get(k - 1)
        paths.set(k, newMaxX !== x ? { prev: lastPath, x, y, length: newMaxX - x } : lastPath)
        if (V.get(k) === seqX.length && V.get(k) - k === seqY.length) {
          foundK = k
          done = true
          break
        }
      }
    }
  }
  let path = paths.get(foundK)
  let lastAligningPosS1 = seqX.length
  let lastAligningPosS2 = seqY.length
  while (true) {
    let endX = 0
    let endY = 0
    let nextX = 0
    let nextY = 0
    let next: HostSnake | null = null
    let hasPath = false
    if (path != null) {
      hasPath = true
      endX = path.x + path.length
      endY = path.y + path.length
      nextX = path.x
      nextY = path.y
      next = path.prev
    }
    if (endX !== lastAligningPosS1 || endY !== lastAligningPosS2) {
      result.push(makeDiffChange(endX, lastAligningPosS1, endY, lastAligningPosS2))
    }
    if (!hasPath) {
      break
    }
    lastAligningPosS1 = nextX
    lastAligningPosS2 = nextY
    path = next
  }
  const reversed: any[] = []
  for (let r = result.length - 1; r >= 0; r--) {
    reversed.push(result[r])
  }
  return reversed
}

export function hostWorkbenchCommand(kind: string) {
  window.dispatchEvent(new CustomEvent("lil-workbench", { detail: String(kind || "") }))
}

export function jsUndefined(): any {
  return undefined
}

export function windowSelf(): typeof globalThis {
  return globalThis
}

export function throwError(msg: string): never {
  throw new Error(msg)
}

export {
  hostPaintEditor,
  hostCoordsToPosition,
  hostStyleFindWidget,
  eventIsComposing,
  eventDeltaY,
} from "./js-host-paint.ts"
