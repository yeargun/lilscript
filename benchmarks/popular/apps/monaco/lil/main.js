// js-host.ts
function setTextContent(el, text) {
  el.textContent = text;
}
function setInnerHTML(el, html) {
  el.innerHTML = html;
}
function setClassName(el, name) {
  el.className = name;
}
function setStyle(el, prop, value) {
  el.style.setProperty(prop, value);
}
function focusElement(el) {
  el.focus();
}
function preventDefault(event) {
  event.preventDefault();
}
function eventKey(event) {
  return event.key;
}
function eventCtrlKey(event) {
  return event.ctrlKey || event.metaKey;
}
function eventShiftKey(event) {
  return event.shiftKey;
}
function eventAltKey(event) {
  return event.altKey;
}
function inputSetValue(el, value) {
  el.value = value;
}
function inputGetValue(el) {
  return el.value;
}
function canvasGetContext2d(canvas) {
  try {
    return canvas.getContext("2d");
  } catch {
    return null;
  }
}
function canvasSetSize(canvas, width, height) {
  canvas.width = width;
  canvas.height = height;
}
function canvasFillRect(ctx, x2, y2, w2, h, color) {
  if (!ctx) {
    return;
  }
  ctx.fillStyle = color;
  ctx.fillRect(x2, y2, w2, h);
}
function setTabIndex(el, value) {
  el.tabIndex = value;
}
var memoryClipboard = "";
function clipboardRead() {
  return memoryClipboard;
}
function clipboardWrite(text) {
  memoryClipboard = text;
  try {
    void navigator.clipboard?.writeText(text);
  } catch {
  }
}
function clipboardReadEvent(event) {
  const data = event.clipboardData?.getData("text/plain");
  if (typeof data === "string" && data.length > 0) {
    memoryClipboard = data;
    return data;
  }
  return memoryClipboard;
}
function clipboardWriteEvent(event, text) {
  memoryClipboard = text;
  event.clipboardData?.setData("text/plain", text);
}
function eventClientX(event) {
  return event.clientX | 0;
}
function eventClientY(event) {
  return event.clientY | 0;
}
function eventDetail(event) {
  return event.detail | 0;
}
function rectLeft(el) {
  return el.getBoundingClientRect().left | 0;
}
function rectTop(el) {
  return el.getBoundingClientRect().top | 0;
}
function setPlaceholder(el, value) {
  el.placeholder = value;
}
function setDisplay(el, value) {
  el.style.display = value;
}
function hostCall(obj, name, a, b, c2) {
  if (obj == null) {
    return void 0;
  }
  const fn = obj[name];
  if (typeof fn !== "function") {
    return void 0;
  }
  try {
    return fn.call(obj, a, b, c2);
  } catch {
    return void 0;
  }
}
function jsArrayLen(value) {
  if (Array.isArray(value)) {
    return value.length;
  }
  if (value && Array.isArray(value.suggestions)) {
    return value.suggestions.length;
  }
  if (value && Array.isArray(value.items)) {
    return value.items.length;
  }
  if (value && Array.isArray(value.lenses)) {
    return value.lenses.length;
  }
  if (value && Array.isArray(value.symbols)) {
    return value.symbols.length;
  }
  if (value && Array.isArray(value.contents)) {
    return value.contents.length;
  }
  return 0;
}
function jsArrayAt(value, index) {
  if (Array.isArray(value)) {
    return value[index];
  }
  const arr = value?.suggestions ?? value?.items ?? value?.lenses ?? value?.symbols ?? value?.contents;
  return Array.isArray(arr) ? arr[index] : void 0;
}
function jsPropString(value, key, fallback = "") {
  if (value == null) {
    return fallback;
  }
  const v2 = value[key];
  if (v2 == null) {
    return fallback;
  }
  if (typeof v2 === "string") {
    return v2;
  }
  if (typeof v2 === "object" && typeof v2.value === "string") {
    return v2.value;
  }
  if (typeof v2 === "object" && typeof v2.label === "string") {
    return v2.label;
  }
  return String(v2);
}
function jsPropInt(value, key, fallback = 0) {
  if (value == null) {
    return fallback;
  }
  const v2 = value[key];
  return typeof v2 === "number" ? v2 | 0 : fallback;
}
function emptyBuf() {
  return Array(1).join("");
}
function concat2(a, b) {
  return a + b;
}
function pushNewBuffer(tree, text, lineStarts) {
  const buf = { buffer: text, lineStarts };
  tree.tmpBuffer = buf;
  tree.buffers.push(buf);
}
function makeNodePos(node, remainder, start) {
  const rem = remainder | 0;
  const off = start | 0;
  const pos = [node, rem, off];
  pos.node = node;
  pos.remainder = rem;
  pos.nodeStartOffset = off;
  return pos;
}
function hostNodeNext(node, sentinel) {
  if (node.right !== sentinel) {
    let cur2 = node.right;
    while (cur2.left !== sentinel) {
      cur2 = cur2.left;
    }
    return cur2;
  }
  let cur = node;
  let guard = 0;
  while (cur.parent !== sentinel && guard < 1e5) {
    guard++;
    if (cur.parent.left === cur) {
      break;
    }
    cur = cur.parent;
  }
  if (cur.parent === sentinel || guard >= 1e5) {
    return sentinel;
  }
  return cur.parent;
}
function hostAccumulatedValue(tree, node, index) {
  if (index < 0) {
    return 0;
  }
  const piece = node.piece;
  const lineStarts = tree.buffers[piece.bufferIndex].lineStarts;
  const expected = piece.start.line + index + 1 | 0;
  if (expected > piece.end.line) {
    return (lineStarts[piece.end.line] | 0) + piece.end.column - (lineStarts[piece.start.line] | 0) - piece.start.column | 0;
  }
  return (lineStarts[expected] | 0) - (lineStarts[piece.start.line] | 0) - piece.start.column | 0;
}
function hostOffsetOfNode(tree, node) {
  let pos = node.size_left | 0;
  let cur = node;
  while (cur !== tree.root) {
    if (cur.parent.right === cur) {
      pos = pos + (cur.parent.size_left | 0) + cur.parent.piece.length | 0;
    }
    cur = cur.parent;
  }
  return pos;
}
function hostNodeAt(tree, offset, sentinel) {
  offset = offset | 0;
  if (tree.cacheValid) {
    const cached = tree.cacheNode;
    const cStart = tree.cacheNodeStartOffset | 0;
    const cLen = cached.piece.length | 0;
    if (cStart <= offset && cStart + cLen >= offset) {
      return makeNodePos(cached, offset - cStart, cStart);
    }
  }
  let rest = offset;
  let x2 = tree.root;
  let nodeStartOffset = 0;
  let guard = 0;
  while (x2 !== sentinel && guard < 1e5) {
    const sizeLeft = x2.size_left | 0;
    const pieceLen = x2.piece.length | 0;
    if (sizeLeft > rest) {
      x2 = x2.left;
    } else if (sizeLeft + pieceLen >= rest) {
      nodeStartOffset = nodeStartOffset + sizeLeft | 0;
      tree.cacheNode = x2;
      tree.cacheNodeStartOffset = nodeStartOffset;
      tree.cacheNodeStartLineNumber = 0;
      tree.cacheHasLine = false;
      tree.cacheValid = true;
      return makeNodePos(x2, rest - sizeLeft, nodeStartOffset);
    } else {
      rest = rest - sizeLeft - pieceLen | 0;
      nodeStartOffset = nodeStartOffset + sizeLeft + pieceLen | 0;
      x2 = x2.right;
    }
    guard++;
  }
  return makeNodePos(sentinel, 0, nodeStartOffset);
}
function hostNodeAt2(tree, lineNumber, column, sentinel) {
  let line = lineNumber | 0;
  let col = column | 0;
  let x2 = tree.root;
  let nodeStartOffset = 0;
  let guard = 0;
  while (x2 !== sentinel && guard < 1e5) {
    const lfLeft = x2.lf_left | 0;
    const pieceLf = x2.piece.lineFeedCnt | 0;
    const xLeft = x2.left;
    if (xLeft !== sentinel && lfLeft >= line - 1) {
      x2 = xLeft;
    } else if (lfLeft + pieceLf > line - 1) {
      const prevAccumulatedValue = hostAccumulatedValue(tree, x2, line - lfLeft - 2);
      const accumulatedValue = hostAccumulatedValue(tree, x2, line - lfLeft - 1);
      nodeStartOffset = nodeStartOffset + x2.size_left | 0;
      let rem = prevAccumulatedValue + col - 1 | 0;
      if (rem > accumulatedValue) {
        rem = accumulatedValue;
      }
      return makeNodePos(x2, rem, nodeStartOffset);
    } else if (lfLeft + pieceLf === line - 1) {
      const prevAccumulatedValue = hostAccumulatedValue(tree, x2, line - lfLeft - 2);
      const pieceLen = x2.piece.length | 0;
      if (prevAccumulatedValue + col - 1 <= pieceLen) {
        return makeNodePos(x2, prevAccumulatedValue + col - 1 | 0, nodeStartOffset);
      }
      col = col - (pieceLen - prevAccumulatedValue) | 0;
      break;
    } else {
      line = line - lfLeft - pieceLf | 0;
      nodeStartOffset = nodeStartOffset + x2.size_left + x2.piece.length | 0;
      x2 = x2.right;
    }
    guard++;
  }
  x2 = hostNodeNext(x2, sentinel);
  let walk = 0;
  while (x2 !== sentinel && walk < 1e5) {
    const pieceLf = x2.piece.lineFeedCnt | 0;
    const pieceLen = x2.piece.length | 0;
    if (pieceLf > 0) {
      const accumulatedValue = hostAccumulatedValue(tree, x2, 0);
      const start = hostOffsetOfNode(tree, x2);
      let rem = col - 1 | 0;
      if (rem > accumulatedValue) {
        rem = accumulatedValue;
      }
      return makeNodePos(x2, rem, start);
    } else if (pieceLen >= col - 1) {
      return makeNodePos(x2, col - 1 | 0, hostOffsetOfNode(tree, x2));
    } else {
      col = col - pieceLen | 0;
    }
    x2 = hostNodeNext(x2, sentinel);
    walk++;
  }
  return makeNodePos(sentinel, 0, nodeStartOffset);
}
function nodeAtStash(tree, offset, sentinel, slot) {
  if (!tree._stash) {
    tree._stash = [null, null];
  }
  tree._stash[slot | 0] = hostNodeAt(tree, offset, sentinel);
}
function nodeAt2Stash(tree, lineNumber, column, sentinel, slot) {
  if (!tree._stash) {
    tree._stash = [null, null];
  }
  tree._stash[slot | 0] = hostNodeAt2(tree, lineNumber, column, sentinel);
}
function stashNode(tree, slot) {
  return tree._stash[slot | 0].node;
}
function stashRem(tree, slot) {
  return tree._stash[slot | 0].remainder | 0;
}
function stashStart(tree, slot) {
  return tree._stash[slot | 0].nodeStartOffset | 0;
}
function hostCursorLine(cursor) {
  return (cursor.line ?? cursor[0] ?? 0) | 0;
}
function hostCursorCol(cursor) {
  return (cursor.column ?? cursor[1] ?? 0) | 0;
}
function hostOffsetInBuffer(tree, bufferIndex, cursor) {
  const lineStarts = tree.buffers[bufferIndex].lineStarts;
  return lineStarts[hostCursorLine(cursor)] + hostCursorCol(cursor) | 0;
}
function hostGetLineFeedCnt(tree, bufferIndex, start, end) {
  const endLine = hostCursorLine(end);
  const endCol = hostCursorCol(end);
  const startLine = hostCursorLine(start);
  if (endCol === 0) {
    return endLine - startLine | 0;
  }
  const lineStarts = tree.buffers[bufferIndex].lineStarts;
  if (endLine === lineStarts.length - 1) {
    return endLine - startLine | 0;
  }
  const nextLineStartOffset = lineStarts[endLine + 1] | 0;
  const endOffset = lineStarts[endLine] + endCol | 0;
  if (nextLineStartOffset > endOffset + 1) {
    return endLine - startLine | 0;
  }
  const previousCharOffset = endOffset - 1 | 0;
  if (tree.buffers[bufferIndex].buffer.charCodeAt(previousCharOffset) === 13) {
    return endLine - startLine + 1 | 0;
  }
  return endLine - startLine | 0;
}
function hostPositionInBuffer(tree, node, remainder) {
  const piece = node.piece;
  const lineStarts = tree.buffers[piece.bufferIndex].lineStarts;
  const startOffset = lineStarts[piece.start.line] + piece.start.column + (remainder | 0) | 0;
  let low = piece.start.line | 0;
  let high = piece.end.line | 0;
  let mid = low;
  let midStart = 0;
  let midStop = 0;
  while (low <= high) {
    mid = low + ((high - low) / 2 | 0) | 0;
    midStart = lineStarts[mid] | 0;
    if (mid === lineStarts.length - 1) {
      midStop = tree.buffers[piece.bufferIndex].buffer.length | 0;
    } else {
      midStop = lineStarts[mid + 1] | 0;
    }
    if (startOffset < midStart) {
      high = mid - 1 | 0;
    } else if (startOffset >= midStop) {
      low = mid + 1 | 0;
    } else {
      break;
    }
  }
  return { line: mid, column: startOffset - midStart | 0 };
}
function hostGetIndexOf(tree, node, accumulatedValue) {
  const piece = node.piece;
  const pos = hostPositionInBuffer(tree, node, accumulatedValue);
  const lineCnt = pos.line - piece.start.line | 0;
  const span = hostOffsetInBuffer(tree, piece.bufferIndex, piece.end) - hostOffsetInBuffer(tree, piece.bufferIndex, piece.start) | 0;
  if (span === (accumulatedValue | 0)) {
    const realLineCnt = hostGetLineFeedCnt(tree, node.piece.bufferIndex, piece.start, pos);
    if (realLineCnt !== lineCnt) {
      return { index: realLineCnt, remainder: 0 };
    }
  }
  return { index: lineCnt, remainder: pos.column | 0 };
}
function hostGetOffsetAt(tree, lineNumber, column, sentinel) {
  let leftLen = 0;
  let x2 = tree.root;
  let line = lineNumber | 0;
  let guard = 0;
  while (x2 !== sentinel && guard < 1e5) {
    guard++;
    if (x2.left !== sentinel && x2.lf_left + 1 >= line) {
      x2 = x2.left;
    } else if (x2.lf_left + x2.piece.lineFeedCnt + 1 >= line) {
      leftLen = leftLen + x2.size_left | 0;
      const acc = hostAccumulatedValue(tree, x2, line - x2.lf_left - 2 | 0);
      return leftLen + acc + (column | 0) - 1 | 0;
    } else {
      line = line - x2.lf_left - x2.piece.lineFeedCnt | 0;
      leftLen = leftLen + x2.size_left + x2.piece.length | 0;
      x2 = x2.right;
    }
  }
  return leftLen;
}
function makePos(line, column) {
  const lineN = line | 0;
  const colN = column | 0;
  const pos = [lineN, colN];
  pos.lineNumber = lineN;
  pos.column = colN;
  return pos;
}
function hostGetPositionAt(tree, offset, sentinel) {
  let rest = offset | 0;
  if (rest < 0) {
    rest = 0;
  }
  let x2 = tree.root;
  let lfCnt = 0;
  const originalOffset = rest;
  let guard = 0;
  while (x2 !== sentinel && guard < 1e5) {
    guard++;
    if (x2.size_left !== 0 && x2.size_left >= rest) {
      x2 = x2.left;
    } else if (x2.size_left + x2.piece.length >= rest) {
      const out = hostGetIndexOf(tree, x2, rest - x2.size_left | 0);
      lfCnt = lfCnt + x2.lf_left + out.index | 0;
      if (out.index === 0) {
        const lineStartOffset = hostGetOffsetAt(tree, lfCnt + 1, 1, sentinel);
        const column = originalOffset - lineStartOffset | 0;
        return makePos(lfCnt + 1, column + 1);
      }
      return makePos(lfCnt + 1, out.remainder + 1);
    } else {
      rest = rest - x2.size_left - x2.piece.length | 0;
      lfCnt = lfCnt + x2.lf_left + x2.piece.lineFeedCnt | 0;
      if (x2.right === sentinel) {
        const lineStartOffset = hostGetOffsetAt(tree, lfCnt + 1, 1, sentinel);
        const column = originalOffset - rest - lineStartOffset | 0;
        return makePos(lfCnt + 1, column + 1);
      }
      x2 = x2.right;
    }
  }
  return makePos(1, 1);
}
function hostCalcSize(node, sentinel) {
  if (node === sentinel) {
    return 0;
  }
  return node.size_left + node.piece.length + hostCalcSize(node.right, sentinel) | 0;
}
function hostCalcLf(node, sentinel) {
  if (node === sentinel) {
    return 0;
  }
  return node.lf_left + node.piece.lineFeedCnt + hostCalcLf(node.right, sentinel) | 0;
}
function hostLeftest(node, sentinel) {
  while (node.left !== sentinel) {
    node = node.left;
  }
  return node;
}
function hostLeftRotate(tree, x2, sentinel) {
  const y2 = x2.right;
  y2.size_left = y2.size_left + x2.size_left + x2.piece.length | 0;
  y2.lf_left = y2.lf_left + x2.lf_left + x2.piece.lineFeedCnt | 0;
  x2.right = y2.left;
  if (y2.left !== sentinel) {
    y2.left.parent = x2;
  }
  y2.parent = x2.parent;
  if (x2.parent === sentinel) {
    tree.root = y2;
  } else if (x2.parent.left === x2) {
    x2.parent.left = y2;
  } else {
    x2.parent.right = y2;
  }
  y2.left = x2;
  x2.parent = y2;
}
function hostRightRotate(tree, y2, sentinel) {
  const x2 = y2.left;
  y2.left = x2.right;
  if (x2.right !== sentinel) {
    x2.right.parent = y2;
  }
  x2.parent = y2.parent;
  y2.size_left = y2.size_left - (x2.size_left + x2.piece.length) | 0;
  y2.lf_left = y2.lf_left - (x2.lf_left + x2.piece.lineFeedCnt) | 0;
  if (y2.parent === sentinel) {
    tree.root = x2;
  } else if (y2 === y2.parent.right) {
    y2.parent.right = x2;
  } else {
    y2.parent.left = x2;
  }
  x2.right = y2;
  y2.parent = x2;
}
function hostUpdateMeta(tree, x2, delta, lfDelta, sentinel) {
  while (x2 !== tree.root && x2 !== sentinel) {
    if (x2.parent.left === x2) {
      x2.parent.size_left = x2.parent.size_left + delta | 0;
      x2.parent.lf_left = x2.parent.lf_left + lfDelta | 0;
    }
    x2 = x2.parent;
  }
}
function hostRecomputeMeta(tree, x2, sentinel) {
  if (x2 === tree.root) {
    return;
  }
  while (x2 !== tree.root && x2 === x2.parent.right) {
    x2 = x2.parent;
  }
  if (x2 === tree.root) {
    return;
  }
  x2 = x2.parent;
  const delta = hostCalcSize(x2.left, sentinel) - x2.size_left | 0;
  const lfDelta = hostCalcLf(x2.left, sentinel) - x2.lf_left | 0;
  x2.size_left = x2.size_left + delta | 0;
  x2.lf_left = x2.lf_left + lfDelta | 0;
  while (x2 !== tree.root && (delta !== 0 || lfDelta !== 0)) {
    if (x2.parent.left === x2) {
      x2.parent.size_left = x2.parent.size_left + delta | 0;
      x2.parent.lf_left = x2.parent.lf_left + lfDelta | 0;
    }
    x2 = x2.parent;
  }
}
function hostDetach(node) {
  node.alive = false;
  node.parent = node;
  node.left = node;
  node.right = node;
}
function rbDeleteTree(tree, z2, sentinel) {
  let y2;
  let x2;
  if (z2.left === sentinel) {
    y2 = z2;
    x2 = y2.right;
  } else if (z2.right === sentinel) {
    y2 = z2;
    x2 = y2.left;
  } else {
    y2 = hostLeftest(z2.right, sentinel);
    x2 = y2.right;
  }
  if (y2 === tree.root) {
    tree.root = x2;
    x2.color = 0;
    hostDetach(z2);
    sentinel.parent = sentinel;
    tree.root.parent = sentinel;
    return;
  }
  const yWasRed = y2.color === 1;
  if (y2 === y2.parent.left) {
    y2.parent.left = x2;
  } else {
    y2.parent.right = x2;
  }
  if (y2 === z2) {
    x2.parent = y2.parent;
    hostRecomputeMeta(tree, x2, sentinel);
  } else {
    if (y2.parent === z2) {
      x2.parent = y2;
    } else {
      x2.parent = y2.parent;
    }
    hostRecomputeMeta(tree, x2, sentinel);
    y2.left = z2.left;
    y2.right = z2.right;
    y2.parent = z2.parent;
    y2.color = z2.color;
    if (z2 === tree.root) {
      tree.root = y2;
    } else if (z2 === z2.parent.left) {
      z2.parent.left = y2;
    } else {
      z2.parent.right = y2;
    }
    if (y2.left !== sentinel) {
      y2.left.parent = y2;
    }
    if (y2.right !== sentinel) {
      y2.right.parent = y2;
    }
    y2.size_left = z2.size_left;
    y2.lf_left = z2.lf_left;
    hostRecomputeMeta(tree, y2, sentinel);
  }
  hostDetach(z2);
  if (x2.parent.left === x2) {
    const newSizeLeft = hostCalcSize(x2, sentinel);
    const newLFLeft = hostCalcLf(x2, sentinel);
    if (newSizeLeft !== x2.parent.size_left || newLFLeft !== x2.parent.lf_left) {
      const delta = newSizeLeft - x2.parent.size_left | 0;
      const lfDelta = newLFLeft - x2.parent.lf_left | 0;
      x2.parent.size_left = newSizeLeft;
      x2.parent.lf_left = newLFLeft;
      hostUpdateMeta(tree, x2.parent, delta, lfDelta, sentinel);
    }
  }
  hostRecomputeMeta(tree, x2.parent, sentinel);
  if (yWasRed) {
    sentinel.parent = sentinel;
    return;
  }
  while (x2 !== tree.root && x2.color === 0) {
    if (x2 === x2.parent.left) {
      let w2 = x2.parent.right;
      if (w2.color === 1) {
        w2.color = 0;
        x2.parent.color = 1;
        hostLeftRotate(tree, x2.parent, sentinel);
        w2 = x2.parent.right;
      }
      if (w2.left.color === 0 && w2.right.color === 0) {
        w2.color = 1;
        x2 = x2.parent;
      } else {
        if (w2.right.color === 0) {
          w2.left.color = 0;
          w2.color = 1;
          hostRightRotate(tree, w2, sentinel);
          w2 = x2.parent.right;
        }
        w2.color = x2.parent.color;
        x2.parent.color = 0;
        w2.right.color = 0;
        hostLeftRotate(tree, x2.parent, sentinel);
        x2 = tree.root;
      }
    } else {
      let w2 = x2.parent.left;
      if (w2.color === 1) {
        w2.color = 0;
        x2.parent.color = 1;
        hostRightRotate(tree, x2.parent, sentinel);
        w2 = x2.parent.left;
      }
      if (w2.left.color === 0 && w2.right.color === 0) {
        w2.color = 1;
        x2 = x2.parent;
      } else {
        if (w2.left.color === 0) {
          w2.right.color = 0;
          w2.color = 1;
          hostLeftRotate(tree, w2, sentinel);
          w2 = x2.parent.left;
        }
        w2.color = x2.parent.color;
        x2.parent.color = 0;
        w2.left.color = 0;
        hostRightRotate(tree, x2.parent, sentinel);
        x2 = tree.root;
      }
    }
  }
  x2.color = 0;
  sentinel.parent = sentinel;
}
function splitLinesHost(text) {
  const lines = [];
  let cur = "";
  for (let i2 = 0; i2 < text.length; i2++) {
    const ch = text.charAt(i2);
    if (ch === "\n") {
      lines.push(cur);
      cur = "";
    } else if (ch !== "\r") {
      cur += ch;
    }
  }
  lines.push(cur);
  return lines;
}
function makeDiffChange(os, oe, ms, me) {
  const change = [os, oe, ms, me];
  change.originalStart = os;
  change.originalEnd = oe;
  change.modifiedStart = ms;
  change.modifiedEnd = me;
  return change;
}
var HostFastInts = class {
  pos = [0, 0, 0, 0, 0, 0, 0, 0];
  neg = [0, 0, 0, 0, 0, 0, 0, 0];
  get(idx) {
    if (idx < 0) {
      const i2 = -idx - 1;
      return i2 >= this.neg.length ? 0 : this.neg[i2];
    }
    return idx >= this.pos.length ? 0 : this.pos[idx];
  }
  set(idx, value) {
    if (idx < 0) {
      const i2 = -idx - 1;
      while (this.neg.length <= i2) {
        this.neg.push(0);
      }
      this.neg[i2] = value;
    } else {
      while (this.pos.length <= idx) {
        this.pos.push(0);
      }
      this.pos[idx] = value;
    }
  }
};
var HostFastPaths = class {
  pos = [];
  neg = [];
  get(idx) {
    if (idx < 0) {
      const i2 = -idx - 1;
      return i2 >= this.neg.length ? null : this.neg[i2];
    }
    return idx >= this.pos.length ? null : this.pos[idx];
  }
  set(idx, value) {
    if (idx < 0) {
      const i2 = -idx - 1;
      while (this.neg.length <= i2) {
        this.neg.push(null);
      }
      this.neg[i2] = value;
    } else {
      while (this.pos.length <= idx) {
        this.pos.push(null);
      }
      this.pos[idx] = value;
    }
  }
};
function hostSnake(seqX, seqY, x2, y2) {
  let xi = x2 | 0;
  let yi = y2 | 0;
  while (xi < seqX.length && yi < seqY.length && seqX[xi] === seqY[yi]) {
    xi++;
    yi++;
  }
  return xi;
}
function hostComputeLineDiff(original, modified) {
  const seqX = splitLinesHost(original);
  const seqY = splitLinesHost(modified);
  const result = [];
  if (seqX.length === 0 && seqY.length === 0) {
    return result;
  }
  if (seqX.length === 0) {
    result.push(makeDiffChange(0, 0, 0, seqY.length));
    return result;
  }
  if (seqY.length === 0) {
    result.push(makeDiffChange(0, seqX.length, 0, 0));
    return result;
  }
  const V2 = new HostFastInts();
  const paths = new HostFastPaths();
  const first = hostSnake(seqX, seqY, 0, 0);
  V2.set(0, first);
  paths.set(0, first === 0 ? null : { prev: null, x: 0, y: 0, length: first });
  let foundK = 0;
  let done = false;
  let d = 0;
  const limit = seqX.length + seqY.length + 2;
  while (!done && d <= limit) {
    d++;
    const lowerBound = -Math.min(d, seqY.length + d % 2);
    const upperBound = Math.min(d, seqX.length + d % 2);
    for (let k2 = lowerBound; k2 <= upperBound; k2 += 2) {
      let maxXofDLineTop = -1;
      if (k2 !== upperBound) {
        maxXofDLineTop = V2.get(k2 + 1);
      }
      let maxXofDLineLeft = -1;
      if (k2 !== lowerBound) {
        maxXofDLineLeft = V2.get(k2 - 1) + 1;
      }
      const x2 = Math.min(Math.max(maxXofDLineTop, maxXofDLineLeft), seqX.length);
      const y2 = x2 - k2;
      if (x2 <= seqX.length && y2 <= seqY.length) {
        const newMaxX = hostSnake(seqX, seqY, x2, y2);
        V2.set(k2, newMaxX);
        const lastPath = x2 === maxXofDLineTop ? paths.get(k2 + 1) : paths.get(k2 - 1);
        paths.set(k2, newMaxX !== x2 ? { prev: lastPath, x: x2, y: y2, length: newMaxX - x2 } : lastPath);
        if (V2.get(k2) === seqX.length && V2.get(k2) - k2 === seqY.length) {
          foundK = k2;
          done = true;
          break;
        }
      }
    }
  }
  let path = paths.get(foundK);
  let lastAligningPosS1 = seqX.length;
  let lastAligningPosS2 = seqY.length;
  while (true) {
    let endX = 0;
    let endY = 0;
    let nextX = 0;
    let nextY = 0;
    let next = null;
    let hasPath = false;
    if (path != null) {
      hasPath = true;
      endX = path.x + path.length;
      endY = path.y + path.length;
      nextX = path.x;
      nextY = path.y;
      next = path.prev;
    }
    if (endX !== lastAligningPosS1 || endY !== lastAligningPosS2) {
      result.push(makeDiffChange(endX, lastAligningPosS1, endY, lastAligningPosS2));
    }
    if (!hasPath) {
      break;
    }
    lastAligningPosS1 = nextX;
    lastAligningPosS2 = nextY;
    path = next;
  }
  const reversed = [];
  for (let r2 = result.length - 1; r2 >= 0; r2--) {
    reversed.push(result[r2]);
  }
  return reversed;
}

// ../../build/monaco-layers/demo-entry.raw.js
var xc = "none";
var yc = "block";
var zc = "background";
var Ac = "\n";
var Bc = "root";
var Cc = "display";
var Dc = "string";
var Ec = "editor.action.addSelectionToNextFindMatch";
var Fc = "position";
var Gc = "white-space";
var Hc = "delimiter.bracket";
var Ic = "";
var Jc = "number";
var Kc = "identifier";
var Lc = "comment";
var Mc = "plaintext";
var Nc = "pointer-events";
var Oc = "editor.action.goToDefinition";
var Pc = "editor.action.goToReferences";
var Qc = "editor.action.triggerSuggest";
var Rc = "</div>";
var Sc = "height";
var Tc = "string.escape";
var Uc = "absolute";
var Vc = "editor.action.commentLine";
var Wc = "editor.action.marker.next";
var Xc = "px";
var Yc = "Enter";
var Zc = "value";
var _c = "keyword";
var $c = "editor.action.gotoLine";
var ad = '[^\\\\"]+';
var bd = "javascript";
var cd = "editor.action.rename";
var dd = " ";
var ed = "top";
var fd = '\\"';
var gd = "left";
var hd = "text";
var id = "wrap";
var jd = "font-size";
var kd = "expandLineSelection";
var ld = '" style="height:';
var md = "color";
var nd = "false";
var od = "width";
var pd = "1px solid #454545";
var qd = "cursorWordSelect";
var rd = "minimap";
var sd = "padding";
var td = "vs-dark";
var vd = "auto";
var wd = "flex";
var xd = "type";
var yd = "monaco-editor ";
var zd = "Escape";
var Ad = "border";
var Cd = "[{}()\\[\\]]";
var Dd = "	";
var Ed = '">';
var Fd = "off";
var Gd = "\\\\.";
var Hd = "class";
var Id = "input";
var Jd = "label";
var Ld = "deleteLeft";
var Md = "insertText";
var Od = "ArrowDown";
var Pd = "[^\\\\']+";
var Rd = "undefined";
var Sd = '[^\\"]+';
var Ud = "hc-black";
var Vd = "inmemory";
var Wd = "language";
var Xd = "markdown";
var Yd = "pre-wrap";
var Zd = "relative";
var _d = "textarea";
var $d = 65535;
var V = "vs";
function r(a, b, c2) {
  b < 0 && (b = 0), c2 > a.length && (c2 = a.length);
  return b >= c2 ? emptyBuf() : a.slice(b, c2);
}
function mb(a) {
  var b = emptyBuf(), c2 = 0;
  while (c2 < a) b += dd, c2 = c2 + 1 | 0;
  return b;
}
function u(a, b, c2, d, e) {
  b > d || b == d && c2 > e ? (a.startLineNumber = d, a.startColumn = e, a.endLineNumber = b, a.endColumn = c2) : (a.startLineNumber = b, a.startColumn = c2, a.endLineNumber = d, a.endColumn = e);
}
function ia(a, b, c2) {
  a.piece = b, a.color = c2, a.size_left = 0, a.lf_left = 0, a.alive = true, a.parent = a, a.left = a, a.right = a;
}
function H(a) {
  for (var e, b = [0], d = a.length, c2 = 0; c2 < d; c2 = c2 + 1 | 0) e = a.charCodeAt(c2), 13 == e ? c2 + 1 < d && 10 == a.charCodeAt(c2 + 1) ? (b.push(c2 + 2 | 0), c2++) : b.push(c2 + 1) : 10 == e && b.push(c2 + 1);
  return b;
}
function Da(a) {
  while (a.left != i) a = a.left;
  return a;
}
function nb(a) {
  while (a.right != i) a = a.right;
  return a;
}
function W(a) {
  if (a.right != i) return Da(a.right);
  for (var c2, b = 0; ; ) {
    c2 = a.parent != i && b < 1e5;
    if (!c2) {
      break;
    }
    b = b + 1 | 0;
    if (a.parent.left == a) break;
    a = a.parent;
  }
  return a.parent == i || b >= 1e5 ? i : a.parent;
}
function Ea(a, b, c2, d) {
  a.root = i, a.buffers = [], a.lineCnt = 1, a.length = 0, a.eol = c2, a.eolLength = 2, a.eolNormalized = d, a.lastChangeBufferPos = { line: 0, column: 0 }, a.cacheNode = i, a.cacheNodeStartOffset = 0, a.cacheNodeStartLineNumber = 0, a.cacheHasLine = false, a.cacheValid = false, a.lastVisitedLineNumber = 0, a.lastVisitedLineValue = emptyBuf(), a.posNode = i, a.posRemainder = 0, a.posStart = 0, a.walkLine = 1, a.walkCol = 1, c2 = { buffer: "", lineStarts: [] }, c2.buffer = emptyBuf(), c2.lineStarts = [0], a.tmpBuffer = c2, ob(a, b);
}
function ob(a, b) {
  var c2 = { buffer: "", lineStarts: [] };
  c2.buffer = emptyBuf(), c2.lineStarts = [0], a.buffers = [c2], a.lastChangeBufferPos = { line: 0, column: 0 }, a.root = i, a.lineCnt = 1, a.length = 0, a.eol = Ac, a.eolLength = 2, a.eolNormalized = true;
  var d = i;
  for (c2 = 0; c2 < b.length; c2++) d = pb(a, d, b[c2], c2 + 1);
  a.cacheValid = false, a.lastVisitedLineNumber = 0, a.lastVisitedLineValue = emptyBuf(), z(a);
}
function pb(a, b, c2, d) {
  if (0 == c2.buffer.length) return b;
  var e = c2.lineStarts;
  0 == e.length && (e = H(c2.buffer), c2.lineStarts = e), d = { bufferIndex: d, start: { line: 0, column: 0 }, end: { line: e.length - 1, column: c2.buffer.length - (e[e.length - 1] | 0) | 0 }, lineFeedCnt: e.length - 1, length: c2.buffer.length }, a.buffers.push(c2);
  return b == i ? y(a, i, d) : y(a, b, d);
}
function Fa(a, b) {
  a != i && (a.parent = b);
}
function Ga(a, b) {
  var c2 = b.right, e = c2.left, d = b.parent;
  c2.size_left = c2.size_left + (b.size_left + b.piece.length | 0) | 0, c2.lf_left = c2.lf_left + (b.lf_left + b.piece.lineFeedCnt | 0) | 0, b.right = e, Fa(e, b), c2.parent = d, d == i ? a.root = c2 : d.left == b ? d.left = c2 : d.right = c2, c2.left = b, b.parent = c2;
}
function Ha(a, b) {
  var c2 = b.left, e = c2.right, d = b.parent;
  b.left = e, Fa(e, b), c2.parent = d, b.size_left = b.size_left - (c2.size_left + c2.piece.length | 0) | 0, b.lf_left = b.lf_left - (c2.lf_left + c2.piece.lineFeedCnt | 0) | 0, d == i ? a.root = c2 : b == d.right ? d.right = c2 : d.left = c2, c2.right = b, b.parent = c2;
}
function Ia(a, b, c2) {
  var d = a.parent;
  d.left == a && (d.size_left = d.size_left + b | 0, d.lf_left = d.lf_left + c2 | 0);
}
function X(a, b, c2, d) {
  while (b != a.root && b != i) Ia(b, c2, d), b = b.parent;
}
var qb = /* @__PURE__ */ (function() {
  function a(b2) {
    return b2 == i ? 0 : (b2.size_left + b2.piece.length | 0) + a(b2.right) | 0;
  }
  function b(a2) {
    return a2 == i ? 0 : (a2.lf_left + a2.piece.lineFeedCnt | 0) + b(a2.right) | 0;
  }
  return function(c2, d) {
    if (d == c2.root) return;
    for (; ; ) {
      var e = d != c2.root && d == d.parent.right;
      if (!e) {
        break;
      }
      d = d.parent;
    }
    if (d == c2.root) return;
    d = d.parent, e = a(d.left) - d.size_left | 0;
    var f = b(d.left) - d.lf_left | 0;
    d.size_left = d.size_left + e | 0, d.lf_left = d.lf_left + f | 0;
    while (d != c2.root && (0 != e || 0 != f)) Ia(d, e, f), d = d.parent;
  };
})();
function Ja(a, b) {
  qb(a, b);
  for (; ; ) {
    var c2 = b != a.root && 1 == b.parent.color;
    if (!c2) {
      break;
    }
    b.parent == b.parent.parent.left ? (c2 = b.parent.parent.right, 1 == c2.color ? (b.parent.color = 0, c2.color = 0, b.parent.parent.color = 1, b = b.parent.parent) : (b == b.parent.right && (b = b.parent, Ga(a, b)), b.parent.color = 0, b.parent.parent.color = 1, Ha(a, b.parent.parent))) : (c2 = b.parent.parent.left, 1 == c2.color ? (b.parent.color = 0, c2.color = 0, b.parent.parent.color = 1, b = b.parent.parent) : (b == b.parent.left && (b = b.parent, Ha(a, b)), b.parent.color = 0, b.parent.parent.color = 1, Ga(a, b.parent.parent)));
  }
  a.root.color = 0;
}
function y(a, b, c2) {
  var d = { parent: null, left: null, right: null, color: 0, piece: null, size_left: 0, lf_left: 0, alive: false };
  ia(d, c2, 1), d.left = i, d.right = i, d.parent = i, d.size_left = 0, d.lf_left = 0, a.root == i ? (a.root = d, d.color = 0) : b.right == i ? (b.right = d, d.parent = b) : (b = Da(b.right), b.left = d, d.parent = b), Ja(a, d);
  return d;
}
function ja(a, b, c2) {
  var d = { parent: null, left: null, right: null, color: 0, piece: null, size_left: 0, lf_left: 0, alive: false };
  ia(d, c2, 1), d.left = i, d.right = i, d.parent = i, d.size_left = 0, d.lf_left = 0, a.root == i ? (a.root = d, d.color = 0) : b.left == i ? (b.left = d, d.parent = b) : (b = nb(b.left), b.right = d, d.parent = b), Ja(a, d);
  return d;
}
function z(a) {
  var b = a.root, c2 = 1, d = 0;
  while (b != i) c2 = c2 + (b.lf_left + b.piece.lineFeedCnt | 0) | 0, d = d + (b.size_left + b.piece.length | 0) | 0, b = b.right;
  a.lineCnt = c2, a.length = d, Y(a, a.length);
}
function Y(a, b) {
  a.cacheValid && (!a.cacheNode.alive || a.cacheNodeStartOffset >= b) && (a.cacheValid = false);
}
function t(a, b, c2) {
  return (a.buffers[b].lineStarts[c2.line] | 0) + c2.column | 0;
}
function I(a, b, c2, h) {
  if (0 == h.column) return h.line - c2.line | 0;
  var d = a.buffers[b].lineStarts;
  if (h.line == d.length - 1) return h.line - c2.line | 0;
  var e = (d[h.line] | 0) + h.column | 0;
  return (d[h.line + 1 | 0] | 0) > (e + 1 | 0) ? h.line - c2.line | 0 : 13 == a.buffers[b].buffer.charCodeAt(e - 1 | 0) ? (h.line - c2.line | 0) + 1 | 0 : h.line - c2.line | 0;
}
function P(a, b, c2) {
  var d = b.piece, g = a.buffers[d.bufferIndex].lineStarts, h = ((g[d.start.line] | 0) + d.start.column | 0) + c2 | 0;
  c2 = d.start.line;
  var i2, e = d.end.line, b = c2, f = 0;
  while (c2 <= e) {
    b = c2 + ((e - c2 | 0) / 2 | 0) | 0, f = g[b] | 0, i2 = b == g.length - 1 ? a.buffers[d.bufferIndex].buffer.length : g[b + 1 | 0] | 0;
    if (h < f) {
      e = b - 1 | 0;
    } else {
      if (h >= i2) {
        c2 = b + 1 | 0;
      } else {
        break;
      }
    }
  }
  return { line: b, column: h - f | 0 };
}
function rb(a, b) {
  if (b == i) return emptyBuf();
  b = b.piece;
  return r(a.buffers[b.bufferIndex].buffer, t(a, b.bufferIndex, b.start), t(a, b.bufferIndex, b.end));
}
function Ka(a) {
  var b = stashNode(a, 0), e = stashNode(a, 1), c2 = stashRem(a, 0), f = stashRem(a, 1);
  if (b == e) {
    var d = t(a, b.piece.bufferIndex, b.piece.start);
    return r(a.buffers[b.piece.bufferIndex].buffer, d + c2 | 0, d + f | 0);
  }
  d = t(a, b.piece.bufferIndex, b.piece.start);
  c2 = r(a.buffers[b.piece.bufferIndex].buffer, d + c2 | 0, d + b.piece.length | 0), b = W(b);
  var g = 0;
  while (b != i && g < 1e5) {
    d = t(a, b.piece.bufferIndex, b.piece.start);
    if (b == e) {
      c2 = concat2(c2, r(a.buffers[b.piece.bufferIndex].buffer, d, d + f | 0));
      break;
    }
    c2 = concat2(c2, r(a.buffers[b.piece.bufferIndex].buffer, d, d + b.piece.length | 0));
    b = W(b), g = g + 1 | 0;
  }
  return c2;
}
function La(a, b) {
  if (b.startLineNumber == b.endLineNumber && b.startColumn == b.endColumn) return emptyBuf();
  nodeAt2Stash(a, b.startLineNumber, b.startColumn, i, 0), nodeAt2Stash(a, b.endLineNumber, b.endColumn, i, 1);
  return Ka(a);
}
function A(a, b) {
  return b == i ? emptyBuf() : concat2(concat2(A(a, b.left), rb(a, b)), A(a, b.right));
}
function Ma(a, b) {
  if (a.lastVisitedLineNumber == b) return a.lastVisitedLineValue;
  a.lastVisitedLineNumber = b;
  if (b == a.lineCnt) {
    a.lastVisitedLineValue = ka(a, b, 0);
  } else {
    if (a.eolNormalized) {
      a.lastVisitedLineValue = ka(a, b, a.eolLength);
    } else {
      a.lastVisitedLineValue = ka(a, b, 0).replace(/\r\n|\r|\n/g, emptyBuf());
    }
  }
  return a.lastVisitedLineValue;
}
function ka(a, h, b) {
  var c2 = hostGetOffsetAt(a, h, 1, i);
  h = h == a.lineCnt ? a.length : hostGetOffsetAt(a, h + 1 | 0, 1, i) - b, h < c2 && (h = c2), nodeAtStash(a, c2, i, 0), nodeAtStash(a, h, i, 1);
  return Ka(a);
}
function sb(a, b) {
  if (b == a.lineCnt) {
    var c2 = a.length;
    return c2 - hostGetOffsetAt(a, b, 1, i) | 0;
  }
  c2 = hostGetOffsetAt(a, b + 1 | 0, 1, i);
  return (c2 - hostGetOffsetAt(a, b, 1, i) | 0) - a.eolLength | 0;
}
function Z(a, b) {
  if (b.length > $d) {
    var e = [];
    while (b.length > $d) {
      var d = b.charCodeAt(65534);
      13 == d || d >= 55296 && d <= 56319 ? (d = r(b, 0, 65534), b = r(b, 65534, b.length)) : (d = r(b, 0, $d), b = r(b, $d, b.length));
      var c2 = H(d);
      e.push({ bufferIndex: a.buffers.length, start: { line: 0, column: 0 }, end: { line: c2.length - 1, column: d.length - (c2[c2.length - 1] | 0) | 0 }, lineFeedCnt: c2.length - 1, length: d.length }), pushNewBuffer(a, d, c2);
    }
    d = H(b);
    e.push({ bufferIndex: a.buffers.length, start: { line: 0, column: 0 }, end: { line: d.length - 1, column: b.length - (d[d.length - 1] | 0) | 0 }, lineFeedCnt: d.length - 1, length: b.length }), pushNewBuffer(a, b, d);
    return e;
  }
  e = a.buffers[0].buffer.length;
  d = H(b);
  var f = a.lastChangeBufferPos;
  if (0 != e) {
    for (c2 = 0; c2 < d.length; c2++) d[c2] = (d[c2] | 0) + e | 0;
  }
  a.buffers[0].lineStarts = a.buffers[0].lineStarts.concat(d.slice(1));
  d = a.buffers[0], d.buffer = concat2(a.buffers[0].buffer, b), d = a.buffers[0].buffer.length, b = a.buffers[0].lineStarts.length - 1, b = { line: b, column: d - (a.buffers[0].lineStarts[b] | 0) | 0 }, d = { bufferIndex: 0, start: f, end: b, lineFeedCnt: I(a, 0, f, b), length: d - e }, a.lastChangeBufferPos = b;
  return [d];
}
function la(a, b, c2) {
  let d = b.piece, f = d.lineFeedCnt, e = I(a, d.bufferIndex, d.start, c2), i2 = e - f | 0;
  f = t(a, d.bufferIndex, c2) - t(a, d.bufferIndex, d.end) | 0, b.piece = { bufferIndex: d.bufferIndex, start: d.start, end: c2, lineFeedCnt: e, length: d.length + f | 0 }, X(a, b, f, i2);
}
function Na(a, b, c2) {
  let d = b.piece, f = d.lineFeedCnt, e = I(a, d.bufferIndex, c2, d.end), h = e - f | 0;
  f = t(a, d.bufferIndex, d.start) - t(a, d.bufferIndex, c2) | 0, b.piece = { bufferIndex: d.bufferIndex, start: c2, end: d.end, lineFeedCnt: e, length: d.length + f | 0 }, X(a, b, f, h);
}
function tb(a, b, g) {
  var e = a.buffers[0].buffer.length, d = a.buffers[0];
  d.buffer = concat2(a.buffers[0].buffer, g), d = H(g);
  for (var f, c2 = 0; c2 < d.length; c2++) d[c2] = (d[c2] | 0) + e | 0;
  a.buffers[0].lineStarts = a.buffers[0].lineStarts.concat(d.slice(1)), d = a.buffers[0].lineStarts.length - 1, d = { line: d, column: a.buffers[0].buffer.length - (a.buffers[0].lineStarts[d] | 0) | 0 }, e = b.piece.length + g.length | 0, f = b.piece.lineFeedCnt, c2 = I(a, 0, b.piece.start, d), b.piece = { bufferIndex: b.piece.bufferIndex, start: b.piece.start, end: d, lineFeedCnt: c2, length: e }, a.lastChangeBufferPos = d, X(a, b, g.length, c2 - f | 0);
}
function ub(a, g, b) {
  var c2 = Z(a, g);
  b = ja(a, b, c2[c2.length - 1]), g = c2.length - 2;
  while (g >= 0) b = ja(a, b, c2[g]), g--;
}
function vb(a, g, b) {
  g = Z(a, g), b = y(a, b, g[0]);
  for (var c2 = 1; c2 < g.length; c2++) b = y(a, b, g[c2]);
}
function wb(a, b, g) {
  a.lastVisitedLineNumber = 0, a.lastVisitedLineValue = emptyBuf();
  if (a.root != i) {
    nodeAtStash(a, b, i, 0);
    var c2 = stashNode(a, 0), h = stashRem(a, 0), f = stashStart(a, 0), d = c2.piece;
    if (0 == d.bufferIndex && d.end.line == a.lastChangeBufferPos.line && d.end.column == a.lastChangeBufferPos.column && (f + d.length | 0) == b && g.length < $d) {
      tb(a, c2, g), z(a);
      return;
    }
    if (f == b) {
      ub(a, g, c2), Y(a, b);
    } else {
      if ((f + d.length | 0) > b) {
        for (b = P(a, c2, h), h = I(a, d.bufferIndex, b, d.end), d = { bufferIndex: d.bufferIndex, start: b, end: d.end, lineFeedCnt: h, length: t(a, d.bufferIndex, d.end) - t(a, d.bufferIndex, b) | 0 }, la(a, c2, b), g = Z(a, g), d.length > 0 && y(a, c2, d), b = 0; b < g.length; b++) c2 = y(a, c2, g[b]);
      } else {
        vb(a, g, c2);
      }
    }
  } else {
    for (b = Z(a, g), g = ja(a, i, b[0]), c2 = 1; c2 < b.length; c2++) g = y(a, g, b[c2]);
  }
  z(a);
}
function xb(a, c2, b) {
  a.lastVisitedLineNumber = 0, a.lastVisitedLineValue = emptyBuf();
  var d, e, f, g, h;
  if (b <= 0 || a.root == i) return;
  nodeAtStash(a, c2, i, 0), nodeAtStash(a, c2 + b | 0, i, 1), d = stashNode(a, 0), e = stashNode(a, 1), f = stashRem(a, 0), g = stashRem(a, 1), h = stashStart(a, 0);
  if (d == e) {
    e = P(a, d, f), f = P(a, d, g);
    if (h == c2) {
      if (b == d.piece.length) {
        rbDeleteTree(a, d, i), z(a);
        return;
      }
      Na(a, d, f);
      Y(a, c2), z(a);
      return;
    }
    if ((h + d.piece.length | 0) == (c2 + b | 0)) {
      la(a, d, e), z(a);
      return;
    }
    c2 = d.piece;
    h = c2.start, b = c2.end, g = I(a, c2.bufferIndex, h, e), h = t(a, c2.bufferIndex, e) - t(a, c2.bufferIndex, h) | 0, d.piece = { bufferIndex: c2.bufferIndex, start: c2.start, end: e, lineFeedCnt: g, length: h }, X(a, d, h - c2.length | 0, g - c2.lineFeedCnt | 0), g = I(a, c2.bufferIndex, f, b), y(a, d, { bufferIndex: c2.bufferIndex, start: f, end: b, lineFeedCnt: g, length: t(a, c2.bufferIndex, b) - t(a, c2.bufferIndex, f) | 0 }), z(a);
    return;
  }
  b = [];
  la(a, d, P(a, d, f)), Y(a, c2), 0 == d.piece.length && b.push(d), Na(a, e, P(a, e, g)), 0 == e.piece.length && b.push(e), c2 = W(d);
  while (c2 != i && c2 != e) b.push(c2), c2 = W(c2);
  for (c2 = 0; c2 < b.length; c2++) rbDeleteTree(a, b[c2], i);
  z(a);
}
function ma(a) {
  var d = a.scheme, b = d + ":";
  (a.authority.length > 0 || "file" == d || d == Vd) && (b = b + "//" + a.authority), b += a.path, a.query.length > 0 && (b = b + "?" + a.query), a.fragment.length > 0 && (b = b + "#" + a.fragment);
  return b;
}
function w(c2) {
  var a = c2.listeners.slice();
  c2 = 0;
  while (c2 < a.length) a[c2](), c2++;
}
function yb(a, b) {
  a.deco = b, a.color = 0, a.maxEndLine = b.range.endLineNumber, a.maxEndColumn = b.range.endColumn, a.alive = true, a.parent = a, a.left = a, a.right = a;
}
function zb(a, b) {
  return a.endLineNumber < b.startLineNumber || a.endLineNumber == b.startLineNumber && a.endColumn < b.startColumn ? false : b.endLineNumber < a.startLineNumber || b.endLineNumber == a.startLineNumber && b.endColumn < a.startColumn ? false : true;
}
function na(a, b, c2, d) {
  if (b == T) return;
  if (b.maxEndLine < c2.startLineNumber || b.maxEndLine == c2.startLineNumber && b.maxEndColumn < c2.startColumn) return;
  na(a, b.left, c2, d), zb(b.deco.range, c2) && d.push(b.deco), (b.deco.range.startLineNumber < c2.endLineNumber || b.deco.range.startLineNumber == c2.endLineNumber && b.deco.range.startColumn <= c2.endColumn) && na(a, b.right, c2, d);
}
function Ab(a) {
  if (0 == a.past.length) return null;
  var b = a.past[a.past.length - 1];
  a.past.splice(a.past.length - 1, 1), a.versionId = a.versionId + 1 | 0;
  return b;
}
function Bb(a) {
  if (0 == a.future.length) return null;
  var b = a.future[a.future.length - 1];
  a.future.splice(a.future.length - 1, 1), a.versionId = a.versionId + 1 | 0;
  return b;
}
function Oa(a, e, d, b, f, g) {
  var h = [];
  if (0 == e.length || g <= 0) return h;
  var i2 = b ? "g" : "gi";
  if (!d) {
    b = emptyBuf();
    var k2, c2 = 0;
    while (c2 < e.length) d = e.charAt(c2), b = "\\" == d || "^" == d || "$" == d || "." == d || "|" == d || "?" == d || "*" == d || "+" == d || "(" == d || ")" == d || "[" == d || "]" == d || "{" == d || "}" == d ? b + "\\" + d : b + d, c2++;
    e = b;
  }
  f && (e = "\\b(?:" + e + ")\\b");
  b = new RegExp(e, i2), d = 1;
  while (d <= a.lineCnt && h.length < g) {
    f = Ma(a, d), e = b.exec(f);
    while (e && h.length < g) c2 = +e.index | 0, e = e[0] + "", i2 = (c2 + e.length | 0) + 1 | 0, c2 = c2 + 1 | 0, k2 = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 }, u(k2, d, c2, d, i2), c2 = [e], e = [null, []], e[0] = k2, e[1] = c2, h.push(e), e = b.exec(f);
    d = d + 1 | 0;
  }
  return h;
}
function oa(a) {
  if (0 == a.length) return false;
  var b = a.charCodeAt(0);
  return b >= 48 && b <= 57 ? true : b >= 65 && b <= 90 ? true : b >= 97 && b <= 122 ? true : "_" == a || "$" == a;
}
function B(a) {
  var d, c2 = a.length, b = 0;
  while (b < c2) {
    d = a.charAt(b);
    if (d != dd && d != Dd) return b;
    b++;
  }
  return c2;
}
function s(d, a) {
  var b = a - 1 | 0;
  b < 0 && (b = 0), b > d.length || (a = b), b = 0;
  while (a > b) {
    if (!oa(d.charAt((a - b | 0) - 1 | 0))) break;
    b = b + 1 | 0;
  }
  var c2 = a - b | 0, e = d.length;
  b = 0;
  while ((a + b | 0) < e) {
    if (!oa(d.charAt(a + b | 0))) break;
    b = b + 1 | 0;
  }
  b = a + b | 0;
  a = ["", 0, 0], a[0] = r(d, c2, b), a[1] = c2 + 1 | 0, a[2] = b + 1 | 0;
  return a;
}
function Cb(a, g, b, c2) {
  let e = g.replace(/\r\n|\r|\n/g, Ac), f = H(e);
  g = { buffer: "", lineStarts: [] }, g.buffer = e, g.lineStarts = f, g = [g], e = { root: null, buffers: [], lineCnt: 0, length: 0, eol: "", eolLength: 0, eolNormalized: false, lastChangeBufferPos: null, cacheNode: null, cacheNodeStartOffset: 0, cacheNodeStartLineNumber: 0, cacheHasLine: false, cacheValid: false, lastVisitedLineNumber: 0, lastVisitedLineValue: "", posNode: null, posRemainder: 0, posStart: 0, walkLine: 0, walkCol: 0, tmpBuffer: null }, Ea(e, g, Ac, true), a.buffer = e, a.languageId = b, a.uri = c2, g = { past: [], future: [], versionId: 0 }, g.past = [], g.future = [], g.versionId = 1, a.stack = g, g = { root: null, nextId: 0, deleteWasY: null, deleteWasX: null, deleteWasRed: false }, g.root = T, g.nextId = 1, g.deleteWasY = T, g.deleteWasX = T, g.deleteWasRed = false, a.decorations = g, g = { listeners: [], disposed: false }, g.listeners = [], g.disposed = false, a.onDidChangeContent = g, a.versionId = 1, a.decoScratchIdx = 0;
}
function q(a) {
  return a.buffer.lineCnt;
}
function k(a, b) {
  return Ma(a.buffer, b);
}
function o(a, b) {
  return sb(a.buffer, b);
}
function l(a, b, d) {
  for (var g, h, j2, k2, c2, l2, m2, e = [], f = 0; f < b.length; f++) e.push(b[f]);
  b = 0;
  while (b < e.length) {
    f = b + 1;
    while (f < e.length) g = e[b], h = e[f], j2 = g.range.startLineNumber, k2 = g.range.startColumn, c2 = h.range.startLineNumber, l2 = h.range.startColumn, (c2 > j2 || c2 == j2 && l2 > k2) && (e[b] = h, e[f] = g), f = f + 1 | 0;
    b++;
  }
  g = [];
  c2 = 0;
  while (c2 < e.length) h = e[c2], b = h.range, j2 = concat2(emptyBuf(), h.text), f = hostGetOffsetAt(a.buffer, b.startLineNumber, b.startColumn, i), k2 = hostGetOffsetAt(a.buffer, b.endLineNumber, b.endColumn, i), l2 = a.buffer, l2 = r(A(l2, l2.root), f, k2), k2 > f && xb(a.buffer, f, k2 - f | 0), j2.length > 0 && wb(a.buffer, f, j2), f = hostGetPositionAt(a.buffer, f + j2.length | 0, i), m2 = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 }, u(m2, b.startLineNumber, b.startColumn, f.lineNumber, f.column), f = concat2(emptyBuf(), l2), b = { range: null, text: "", identifier: 0 }, b.range = m2, b.text = f, b.identifier = h.identifier, g.push(b), c2++;
  a.versionId = a.versionId + 1 | 0, d && (b = a.stack, b.past.push(g), b.future = [], b.versionId = b.versionId + 1 | 0), w(a.onDidChangeContent);
  return g;
}
function Pa(a) {
  var b = Ab(a.stack);
  if (!b) return;
  a.stack.future.push(l(a, b, false));
}
function Qa(a) {
  var b = Bb(a.stack);
  if (!b) return;
  a.stack.past.push(l(a, b, false));
}
function p(a, b, f, c2, e) {
  let d = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
  u(d, a, b, f, c2), b = concat2(emptyBuf(), e), a = { range: null, text: "", identifier: 0 }, a.range = d, a.text = b, a.identifier = 0;
  return a;
}
function C(a, b, c2, d, e) {
  let f = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
  u(f, b, c2, d, e), a.startLineNumber = f.startLineNumber, a.startColumn = f.startColumn, a.endLineNumber = f.endLineNumber, a.endColumn = f.endColumn, a.selectionStartLineNumber = b, a.selectionStartColumn = c2, a.positionLineNumber = d, a.positionColumn = e;
}
function N(a, b, c2, d, e) {
  a.languageId = b, a.tokenPostfix = c2, a.defaultToken = d, a.keywords = e, a.stateNames = [], a.stateRules = [], a.maxStack = 100;
}
function D(a, b, c2, d, e, f) {
  a.id = b, a.lexer = c2, a.lineComment = d, a.blockCommentStart = e, a.blockCommentEnd = f, a.tokensProvider = void 0;
}
var pa = /* @__PURE__ */ (function() {
  function a(a2, b2) {
    if (a2.stateNames.indexOf(b2) >= 0) return;
    a2.stateNames.push(b2), a2.stateRules.push([]);
  }
  function b() {
    return "[ \\t\\r\\n]+";
  }
  function c2() {
    return "delimiter";
  }
  function d(b2, c3, d2, e2, f2, g2, h2) {
    a(b2, c3);
    var v2 = b2.stateNames, i3 = v2.indexOf(c3);
    (0 == d2.length || "^" != d2.charAt(0)) && (d2 = "^(?:" + d2 + ")"), c3 = b2.stateRules[i3], b2 = { pattern: new RegExp(), lineStart: false, kind: 0, token: "", next: "" }, b2.pattern = new RegExp(d2, Ic), b2.lineStart = h2, b2.kind = e2, b2.token = f2, b2.next = g2, c3.push(b2);
  }
  function e() {
    let c3 = "break case catch class continue const constructor debugger default delete do else export extends false finally for from function get if import in instanceof let new null return set static super switch symbol this throw true try typeof undefined var void while with yield async await of type interface enum implements package private protected public readonly namespace abstract as asserts keyof infer never unknown any boolean number string unique".split(" "), b2 = { languageId: "", tokenPostfix: "", defaultToken: "", keywords: [], stateNames: [], stateRules: [], maxStack: 0 };
    N(b2, bd, ".js", "source", c3), a(b2, Bc), f(b2);
    return b2;
  }
  function f(e2) {
    let f2 = false;
    d(e2, Bc, b(), 0, Ic, Ic, f2), d(e2, Bc, "//.*", 0, Lc, Ic, f2), d(e2, Bc, "/\\*", 1, Lc, Lc, f2), d(e2, Bc, fd, 1, Dc, Dc, f2);
    let o2 = "'", k3 = "stringS";
    d(e2, Bc, o2, 1, Dc, k3, f2);
    let p2 = "`", l3 = "stringT";
    d(e2, Bc, p2, 1, Dc, l3, f2), d(e2, Bc, "0[xX][0-9a-fA-F]+", 0, Jc, Ic, f2), d(e2, Bc, "\\d+\\.\\d+([eE][+\\-]?\\d+)?", 0, Jc, Ic, f2), d(e2, Bc, "\\d+", 0, Jc, Ic, f2), d(e2, Bc, "[a-zA-Z_$][\\w$]*", 4, Kc, Ic, f2), d(e2, Bc, Cd, 0, Hc, Ic, f2), d(e2, Bc, "[;,.]", 0, c2(), Ic, f2), d(e2, Bc, "[+\\-*/%&|^~<>=!?:]+", 0, c2(), Ic, f2), a(e2, Lc), d(e2, Lc, "\\*/", 2, Lc, Ic, f2), d(e2, Lc, "[^*]+", 0, Lc, Ic, f2), d(e2, Lc, "\\*", 0, Lc, Ic, f2), a(e2, Dc), d(e2, Dc, Gd, 0, Tc, Ic, f2), d(e2, Dc, fd, 2, Dc, Ic, f2), d(e2, Dc, ad, 0, Dc, Ic, f2), a(e2, k3), d(e2, k3, Gd, 0, Tc, Ic, f2), d(e2, k3, o2, 2, Dc, Ic, f2), d(e2, k3, Pd, 0, Dc, Ic, f2), a(e2, l3), d(e2, l3, Gd, 0, Tc, Ic, f2), d(e2, l3, p2, 2, Dc, Ic, f2), d(e2, l3, "[^\\\\`]+", 0, Dc, Ic, f2);
  }
  function g() {
    let g2 = ["true", nd, "null"], e2 = { languageId: "", tokenPostfix: "", defaultToken: "", keywords: [], stateNames: [], stateRules: [], maxStack: 0 };
    N(e2, "json", ".json", Ic, g2), a(e2, Bc), d(e2, Bc, b(), 0, Ic, Ic, false), d(e2, Bc, "[{}\\[\\]]", 0, Hc, Ic, false), d(e2, Bc, "[:,]", 0, c2(), Ic, false), d(e2, Bc, "true|false|null", 4, _c, Ic, false), d(e2, Bc, "-?\\d+(\\.\\d+)?([eE][+\\-]?\\d+)?", 0, Jc, Ic, false), d(e2, Bc, fd, 1, Dc, Dc, false), a(e2, Dc), d(e2, Dc, Gd, 0, Tc, Ic, false), d(e2, Dc, fd, 2, Dc, Ic, false), d(e2, Dc, ad, 0, Dc, Ic, false);
    return e2;
  }
  function h() {
    let f2 = "False None True and as assert async await break class continue def del elif else except finally for from global if import in is lambda nonlocal not or pass raise return try while with yield match case".split(" "), e2 = { languageId: "", tokenPostfix: "", defaultToken: "", keywords: [], stateNames: [], stateRules: [], maxStack: 0 };
    N(e2, "python", ".python", Ic, f2), a(e2, Bc), f2 = false, d(e2, Bc, b(), 0, Ic, Ic, f2), d(e2, Bc, "#.*", 0, Lc, Ic, f2);
    let o2 = '\\"\\"\\"', j3 = "tstring";
    d(e2, Bc, o2, 1, Dc, j3, f2);
    let p2 = "'''", k3 = "tstringS";
    d(e2, Bc, p2, 1, Dc, k3, f2), d(e2, Bc, fd, 1, Dc, Dc, f2);
    let n2 = "'", l3 = "stringS";
    d(e2, Bc, n2, 1, Dc, l3, f2), d(e2, Bc, "\\d+\\.\\d+", 0, Jc, Ic, f2), d(e2, Bc, "\\d+", 0, Jc, Ic, f2), d(e2, Bc, "[a-zA-Z_][\\w]*", 4, Kc, Ic, f2), d(e2, Bc, Cd, 0, Hc, Ic, f2), d(e2, Bc, "[:;,.=+\\-*/%<>!&|^~]+", 0, c2(), Ic, f2), a(e2, Dc), d(e2, Dc, Gd, 0, Tc, Ic, f2), d(e2, Dc, fd, 2, Dc, Ic, f2), d(e2, Dc, ad, 0, Dc, Ic, f2), a(e2, l3), d(e2, l3, Gd, 0, Tc, Ic, f2), d(e2, l3, n2, 2, Dc, Ic, f2), d(e2, l3, Pd, 0, Dc, Ic, f2), a(e2, j3), d(e2, j3, o2, 2, Dc, Ic, f2), d(e2, j3, '[^"]+', 0, Dc, Ic, f2), d(e2, j3, fd, 0, Dc, Ic, f2), a(e2, k3), d(e2, k3, p2, 2, Dc, Ic, f2), d(e2, k3, "[^']+", 0, Dc, Ic, f2), d(e2, k3, n2, 0, Dc, Ic, f2);
    return e2;
  }
  function i2() {
    let g2 = "html", h2 = "html head body div span script style link meta title p a ul ol li table tr td th form input button img h1 h2 h3 h4 h5 h6 section article nav footer header main pre code textarea select option".split(" "), e2 = { languageId: "", tokenPostfix: "", defaultToken: "", keywords: [], stateNames: [], stateRules: [], maxStack: 0 };
    N(e2, g2, ".html", Ic, h2);
    let j3 = Bc;
    a(e2, Bc), g2 = false, d(e2, Bc, b(), 0, Ic, Ic, g2);
    let i3 = Lc;
    d(e2, Bc, "<!--", 1, Lc, Lc, g2), h2 = "tag", d(e2, Bc, "</?[a-zA-Z][\\w:-]*", 1, h2, h2, g2), d(e2, Bc, "[^<]+", 0, Ic, Ic, g2), a(e2, Lc), d(e2, Lc, "-->", 2, Lc, Ic, g2), d(e2, Lc, "[^-]+", 0, Lc, Ic, g2), d(e2, Lc, "-", 0, Lc, Ic, g2), a(e2, h2), d(e2, h2, "/?>", 2, h2, Ic, g2), d(e2, h2, b(), 0, Ic, Ic, g2), d(e2, h2, "[a-zA-Z_:][\\w:.-]*", 0, "attribute.name", Ic, g2), d(e2, h2, "=", 0, c2(), Ic, g2), i3 = "attribute.value", j3 = "attr", d(e2, h2, fd, 1, i3, j3, g2);
    let m2 = "'", k3 = "attrS";
    d(e2, h2, m2, 1, i3, k3, g2), a(e2, j3), d(e2, j3, fd, 2, i3, Ic, g2), d(e2, j3, Sd, 0, i3, Ic, g2), a(e2, k3), d(e2, k3, m2, 2, i3, Ic, g2), d(e2, k3, "[^']+", 0, i3, Ic, g2);
    return e2;
  }
  function j2() {
    let g2 = ["important", md, zc, "margin", sd, Ad, Cc, wd, "grid", Fc, ed, gd, "right", "bottom", od, Sc, "font", hd, "align", "justify", "content", Uc, Zd, "fixed", yc, "inline", xc, vd, "inherit", "initial"], e2 = { languageId: "", tokenPostfix: "", defaultToken: "", keywords: [], stateNames: [], stateRules: [], maxStack: 0 };
    N(e2, "css", ".css", Ic, g2), a(e2, Bc), g2 = false, d(e2, Bc, b(), 0, Ic, Ic, g2), d(e2, Bc, "/\\*", 1, Lc, Lc, g2), d(e2, Bc, fd, 1, Dc, Dc, g2), d(e2, Bc, "#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})\\b", 0, "number.hex", Ic, g2), d(e2, Bc, "-?\\d+(\\.\\d+)?(px|em|rem|%|vh|vw|pt|ex)?", 0, Jc, Ic, g2), d(e2, Bc, "[a-zA-Z_-][\\w-]*", 4, Kc, Ic, g2), d(e2, Bc, "[{}();:]", 0, c2(), Ic, g2), d(e2, Bc, "[.,#>\\[\\]+~*]", 0, c2(), Ic, g2), a(e2, Lc), d(e2, Lc, "\\*/", 2, Lc, Ic, g2), d(e2, Lc, "[^*]+", 0, Lc, Ic, g2), d(e2, Lc, "\\*", 0, Lc, Ic, g2), a(e2, Dc), d(e2, Dc, fd, 2, Dc, Ic, g2), d(e2, Dc, Sd, 0, Dc, Ic, g2);
    return e2;
  }
  function k2() {
    let e2 = [], b2 = { languageId: "", tokenPostfix: "", defaultToken: "", keywords: [], stateNames: [], stateRules: [], maxStack: 0 };
    N(b2, Xd, ".md", Ic, e2), a(b2, Bc);
    let f2 = _c;
    d(b2, Bc, "^#{1,6}[ \\t].*$", 0, _c, Ic, true), d(b2, Bc, "^\\s*[-*+]\\s+", 0, _c, Ic, true);
    let h2 = "`+";
    f2 = "code", d(b2, Bc, h2, 1, Dc, f2, false), d(b2, Bc, "\\*\\*[^*]+\\*\\*", 0, "strong", Ic, false), d(b2, Bc, "\\*[^*]+\\*", 0, "emphasis", Ic, false), d(b2, Bc, "\\[[^\\]]+\\]\\([^\\)]+\\)", 0, "string.link", Ic, false), d(b2, Bc, "[^`*\\[#]+", 0, Ic, Ic, false), d(b2, Bc, ".", 0, Ic, Ic, false), a(b2, f2), d(b2, f2, h2, 2, Dc, Ic, false), d(b2, f2, "[^`]+", 0, Dc, Ic, false);
    return b2;
  }
  function l2(a2) {
    var c3 = 0;
    while (c3 < G.length) {
      if (G[c3].id == a2.id) {
        G[c3] = a2;
        return;
      }
      c3++;
    }
    G.push(a2);
  }
  return function() {
    if (Aa) return;
    Aa = true;
    var d2 = e(), a2 = "//", b2 = "/*", c3 = "*/", f2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    D(f2, bd, d2, a2, b2, c3), l2(f2), f2 = "typescript", d2 = e(), d2.languageId = f2, d2.tokenPostfix = ".ts";
    var m2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null };
    D(m2, f2, d2, a2, b2, c3), l2(m2), d2 = g(), f2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, D(f2, "json", d2, a2, b2, c3), l2(f2), d2 = h(), a2 = '"""', f2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, D(f2, "python", d2, "#", a2, a2), l2(f2), d2 = i2(), f2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, D(f2, "html", d2, Ic, "<!--", "-->"), l2(f2), d2 = j2(), f2 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, D(f2, "css", d2, Ic, b2, c3), l2(f2), b2 = k2(), c3 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, D(c3, Xd, b2, Ic, Ic, Ic), l2(c3), b2 = e(), c3 = { id: "", lexer: null, lineComment: "", blockCommentStart: "", blockCommentEnd: "", tokensProvider: null }, D(c3, Mc, b2, Ic, Ic, Ic), l2(c3);
  };
})();
function Q(a) {
  pa();
  var c2 = 0;
  while (c2 < G.length) {
    if (G[c2].id == a) return G[c2];
    c2++;
  }
  return null;
}
function J(a, b, c2) {
  return Oa(a.buffer, b, false, true, c2, 1e3);
}
function Ra(a) {
  var c2, f, g, i2, h, d = [], e = q(a), b = 1;
  while (b <= e) {
    c2 = k(a, b), f = B(c2);
    if (f == c2.length) {
      b = b + 1 | 0;
      continue;
    }
    h = b;
    c2 = b + 1 | 0;
    while (c2 <= e) {
      g = k(a, c2), i2 = B(g);
      if (i2 == g.length) {
        c2 = c2 + 1 | 0;
        continue;
      }
      if (i2 > f) {
        h = c2, c2 = c2 + 1 | 0;
        continue;
      }
      break;
    }
    h > b && (c2 = { startLine: 0, endLine: 0, collapsed: false }, c2.startLine = b, c2.endLine = h, c2.collapsed = false, d.push(c2));
    b = b + 1 | 0;
  }
  return d;
}
function Sa(a) {
  var d, b = emptyBuf(), c2 = 0;
  while (c2 < a.length) {
    if ("$" == a.charAt(c2) && c2 + 1 < a.length) {
      d = a.charAt(c2 + 1);
      if ("0" == d || "1" == d || "2" == d || "3" == d) {
        c2 = c2 + 2 | 0;
        continue;
      }
      if ("{" == d) {
        c2 = c2 + 2 | 0;
        while (c2 < a.length && "}" != a.charAt(c2)) c2 = c2 + 1 | 0;
        c2 = c2 + 1 | 0;
        continue;
      }
    }
    b += a.charAt(c2);
    c2++;
  }
  return b;
}
function Ta(a, b, c2) {
  c2 = Q(c2);
  var d, e = "//";
  c2 && c2.lineComment.length > 0 && (e = c2.lineComment), c2 = k(a, b);
  var f = B(c2);
  d = r(c2, f, c2.length), 0 == d.indexOf(e) ? (e = e.length, d.length > e && d.charAt(e) == dd && (e = e + 1 | 0), l(a, [p(b, 1, b, c2.length + 1 | 0, r(c2, 0, f) + r(d, e, d.length))], true)) : l(a, [p(b, 1, b, c2.length + 1 | 0, r(c2, 0, f) + e + dd + d)], true);
}
function Ua(a, b) {
  b = s(k(a, b.lineNumber), b.column)[0];
  return 0 == b.length ? [] : Oa(a.buffer, b, false, true, true, 200);
}
function Db(g) {
  var d, e, a = emptyBuf(), b = 0, c2 = 0;
  while (c2 < g.length) {
    d = g.charAt(c2);
    if ("{" == d) {
      a = a + d + Ac, b = b + 1 | 0, d = 0;
      while (d < b) a += "  ", d = d + 1 | 0;
    } else {
      if ("}" == d) {
        b > 0 && (b = b - 1 | 0), a += Ac, e = 0;
        while (e < b) a += "  ", e = e + 1 | 0;
        a += d;
      } else {
        a += d;
      }
    }
    c2++;
  }
  return a;
}
function qa(a, b) {
  var e, d = [], c2 = 0;
  while (c2 < Ba.length && (b <= 0 || d.length < b)) e = Ba[c2], (0 == a.length || e[7] == a) && d.push(e), c2++;
  return d;
}
function Eb(a) {
  return a >= 8 ? "squiggly-error" : a >= 4 ? "squiggly-warning" : a >= 2 ? "squiggly-info" : "squiggly-hint";
}
function Fb(a, b, c2) {
  a.root = b, a.model = c2, b = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 }, C(b, 1, 1, 1, 1), a.selection = b, a.extraCursors = [], a.scrollTop = 0, a.lineHeight = 19, a.charWidth = 8, a.width = 800, a.height = 400, a.showLineNumbers = true, a.showMinimap = true, a.theme = "vs", a.languageId = c2.languageId, a.readOnly = false, a.fontSize = 14, a.tabSize = 4, a.insertSpaces = true, a.wordWrap = false, a.mouseSelecting = false, a.mouseAnchorLine = 1, a.mouseAnchorColumn = 1, a.folds = [], a.collapsed = [], a.findOpen = false, a.findQuery = Ic, a.replaceQuery = Ic, a.findIndex = 0, a.suggestOpen = false, a.suggestItems = [], a.suggestIndex = 0, a.hoverOpen = false, a.contextOpen = false, a.gotoOpen = false, a.renameOpen = false, a.renameWord = Ic, a.modelFacade = void 0, b = "div", a.overflow = document.createElement(b), a.margin = document.createElement(b), a.scrollable = document.createElement(b), a.linesHost = document.createElement(b), a.textarea = document.createElement(_d), a.minimapCanvas = document.createElement("canvas"), a.widgetsHost = document.createElement(b), a.cursorEl = document.createElement(b), a.selectionHost = document.createElement(b), a.findWidget = document.createElement(b), a.findInput = document.createElement(Id), a.replaceInput = document.createElement(Id), a.suggestWidget = document.createElement(b), a.hoverWidget = document.createElement(b), a.contextWidget = document.createElement(b), a.gotoWidget = document.createElement(b), a.gotoInput = document.createElement(Id), a.renameWidget = document.createElement(b), a.renameInput = document.createElement(Id), a.paramWidget = document.createElement(b), a.stickyWidget = document.createElement(b), Gb(a);
}
function Gb(a) {
  setClassName(a.root, yd + a.theme);
  var d = Zd;
  setStyle(a.root, Fc, Zd);
  var e = "overflow";
  setStyle(a.root, e, "hidden"), setStyle(a.root, "font-family", "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace");
  var b = Xc;
  setStyle(a.root, jd, a.fontSize.toString(10) + Xc), setStyle(a.root, "line-height", a.lineHeight.toString(10) + Xc), setClassName(a.overflow, "overflow-guard"), setStyle(a.overflow, Cc, wd), setStyle(a.overflow, Sc, "100%"), setClassName(a.margin, "margin"), setStyle(a.margin, od, "56px"), setStyle(a.margin, wd, "0 0 56px"), setStyle(a.margin, "user-select", xc), setClassName(a.scrollable, "monaco-scrollable-element"), setStyle(a.scrollable, wd, "1 1 auto"), setStyle(a.scrollable, e, vd), setStyle(a.scrollable, Fc, Zd), setClassName(a.linesHost, "view-lines"), a.wordWrap ? (setStyle(a.linesHost, Gc, Yd), a.textarea.setAttribute(id, "soft")) : (setStyle(a.linesHost, Gc, "pre"), a.textarea.setAttribute(id, Fd)), setStyle(a.selectionHost, Fc, Uc);
  var i2 = "inset";
  d = "0", setStyle(a.selectionHost, i2, d), setStyle(a.selectionHost, Nc, xc), setClassName(a.cursorEl, "cursor"), setStyle(a.cursorEl, Fc, Uc), setStyle(a.cursorEl, od, "2px"), setStyle(a.cursorEl, Sc, a.lineHeight.toString(10) + Xc), setStyle(a.cursorEl, zc, "#aeafad"), setStyle(a.cursorEl, Nc, xc), a.textarea.setAttribute(Hd, "inputarea");
  var h = Fd;
  a.textarea.setAttribute(id, Fd), a.textarea.setAttribute("autocorrect", Fd), a.textarea.setAttribute("autocapitalize", Fd), a.textarea.setAttribute("spellcheck", nd), setTabIndex(a.textarea, 0), setStyle(a.textarea, Fc, Uc), h = "1px", setStyle(a.textarea, od, h), setStyle(a.textarea, Sc, h), setStyle(a.textarea, "opacity", d), setClassName(a.minimapCanvas, rd), setStyle(a.minimapCanvas, od, "64px"), setStyle(a.minimapCanvas, wd, "0 0 64px"), setClassName(a.widgetsHost, "overflowingContentWidgets"), setStyle(a.widgetsHost, Fc, Uc), setStyle(a.widgetsHost, i2, d), setStyle(a.widgetsHost, Nc, xc), Hb(a), K(a, a.suggestWidget, "suggest-widget"), K(a, a.hoverWidget, "monaco-hover"), K(a, a.contextWidget, "context-view"), K(a, a.gotoWidget, "goto-line-widget"), setPlaceholder(a.gotoInput, "Go to line"), a.gotoWidget.appendChild(a.gotoInput), K(a, a.renameWidget, "rename-box"), a.renameWidget.appendChild(a.renameInput), K(a, a.paramWidget, "parameter-hints-widget"), K(a, a.stickyWidget, "sticky-widget"), a.scrollable.appendChild(a.selectionHost), a.scrollable.appendChild(a.linesHost), a.scrollable.appendChild(a.cursorEl), a.overflow.appendChild(a.margin), a.overflow.appendChild(a.scrollable), a.overflow.appendChild(a.minimapCanvas), a.root.appendChild(a.overflow), a.root.appendChild(a.textarea), a.root.appendChild(a.widgetsHost), R(a), setDisplay(a.findWidget, xc), setDisplay(a.suggestWidget, xc), setDisplay(a.hoverWidget, xc), setDisplay(a.contextWidget, xc), setDisplay(a.gotoWidget, xc), setDisplay(a.renameWidget, xc), setDisplay(a.paramWidget, xc), b = document.createElement("style"), setTextContent(b, ".mtk-keyword{color:#569cd6}.mtk-string{color:#ce9178}.mtk-comment{color:#6a9955}.mtk-number{color:#b5cea8}.mtk-tag{color:#569cd6}.mtk-attr{color:#9cdcfe}.squiggly-error{text-decoration:underline wavy #f14c4c}.squiggly-warning{text-decoration:underline wavy #cca700}.squiggly-info{text-decoration:underline wavy #3794ff}.squiggly-hint{text-decoration:underline dotted #eeeeee}.folding{cursor:pointer;display:inline-block;width:12px}"), a.root.appendChild(b);
}
function K(a, f, b) {
  setClassName(f, b), setStyle(f, Fc, Uc), setStyle(f, Nc, vd), setStyle(f, "z-index", "40"), setStyle(f, Cc, xc), setStyle(f, "max-width", "420px"), setStyle(f, sd, "6px 8px"), setStyle(f, Ad, pd), setStyle(f, zc, "#252526"), setStyle(f, md, "#cccccc"), setStyle(f, jd, "12px"), a.widgetsHost.appendChild(f);
}
function Hb(a) {
  setClassName(a.findWidget, "editor-widget find-widget"), setStyle(a.findWidget, Fc, Uc);
  let b = "8px";
  setStyle(a.findWidget, ed, b), setStyle(a.findWidget, "right", "72px"), setStyle(a.findWidget, "z-index", "50"), setStyle(a.findWidget, Nc, vd), setStyle(a.findWidget, Cc, xc), setStyle(a.findWidget, zc, "#252526"), setStyle(a.findWidget, Ad, pd), setStyle(a.findWidget, sd, b), setPlaceholder(a.findInput, "Find"), setPlaceholder(a.replaceInput, "Replace"), b = "data-monaco", a.findInput.setAttribute(b, "find"), a.replaceInput.setAttribute(b, "replace"), a.findWidget.appendChild(a.findInput), a.findWidget.appendChild(a.replaceInput), a.widgetsHost.appendChild(a.findWidget);
}
function R(a) {
  var d = a.root, f = a.theme;
  setClassName(d, yd + f), setStyle(d, jd, a.fontSize.toString(10) + Xc);
  if (f == td || f == Ud) {
    setStyle(d, zc, "#1e1e1e"), setStyle(d, md, "#d4d4d4"), setStyle(a.cursorEl, zc, "#aeafad");
  } else {
    setStyle(d, zc, "#fffffe");
    var c2 = "#000000";
    setStyle(d, md, c2), setStyle(a.cursorEl, zc, c2);
  }
  a.wordWrap ? (setStyle(a.linesHost, Gc, Yd), a.textarea.setAttribute(id, "soft")) : (setStyle(a.linesHost, Gc, "pre"), a.textarea.setAttribute(id, Fd));
}
function Va(a) {
  var d = (a.scrollTop / a.lineHeight | 0) + 1 | 0;
  return d < 1 ? 1 : $(a, d);
}
function Ib(a) {
  var b = (Va(a) + (a.height / a.lineHeight | 0) | 0) + 8 | 0;
  a = q(a.model);
  return b > a ? a : b;
}
function _(a, d) {
  var b, c2 = 0;
  while (c2 < a.folds.length) {
    b = a.folds[c2];
    if (b.collapsed && d > b.startLine && d <= b.endLine) return true;
    c2++;
  }
  return false;
}
function $(a, b) {
  var c2 = q(a.model);
  while (b <= c2 && _(a, b)) b = b + 1 | 0;
  return b > c2 ? c2 : b;
}
function aa(a, d) {
  var c2 = 0;
  while (c2 < a.folds.length) {
    if (a.folds[c2].startLine == d) return a.folds[c2];
    c2++;
  }
  return null;
}
function ra(a) {
  var d, b = Ra(a.model), c2 = 0;
  while (c2 < b.length) d = aa(a, b[c2].startLine), d && (b[c2].collapsed = d.collapsed), c2++;
  a.folds = b;
}
function ba(a, d) {
  var b = aa(a, d);
  b || (ra(a), b = aa(a, d));
  if (!b) return;
  b.collapsed = !b.collapsed, j(a);
}
function Wa(a, b) {
  ra(a);
  var c2 = 0;
  while (c2 < a.folds.length) a.folds[c2].collapsed = b, c2++;
  j(a);
}
function Jb(a) {
  return a.indexOf(Lc) >= 0 ? "mtk-comment" : a.indexOf(Dc) >= 0 ? "mtk-string" : a.indexOf(_c) >= 0 ? "mtk-keyword" : a.indexOf(Jc) >= 0 ? "mtk-number" : a.indexOf("tag") >= 0 ? "mtk-tag" : a.indexOf("attribute") >= 0 ? "mtk-attr" : "mtk";
}
function Xa(a) {
  var d, b = emptyBuf(), c2 = 0;
  while (c2 < a.length) d = a.charAt(c2), "<" == d ? b = b + "&lt;" : ">" == d ? b = b + "&gt;" : "&" == d ? b = b + "&amp;" : b += d, c2++;
  return b;
}
function Kb(a, b) {
  if (0 == b.length) {
    return 0 == a.length ? "<span>&nbsp;</span>" : '<span class="mtk">' + Xa(a) + "</span>";
  }
  var h, d = emptyBuf(), c2 = 0;
  while (c2 < b.length) h = a.length, c2 + 1 < b.length && (h = b[c2 + 1][0]), h = r(a, b[c2][0], h), d += '<span class="', d = d + Jb(b[c2][1]) + Ed + Xa(h) + "</span>", c2++;
  return d;
}
function Lb(a) {
  if (!a.showMinimap) {
    setStyle(a.minimapCanvas, Cc, xc);
    return;
  }
  setStyle(a.minimapCanvas, Cc, yc);
  var c2 = a.height;
  c2 < 1 && (c2 = 1);
  var t2 = a.minimapCanvas;
  canvasSetSize(t2, 64, c2);
  var e = canvasGetContext2d(t2), b = "#f3f3f3", f = "#6e6e6e";
  (a.theme == td || a.theme == Ud) && (b = "#1e1e1e", f = "#5a5a5a"), canvasFillRect(e, 0, 0, 64, c2, b);
  var d = q(a.model);
  if (d < 1) return;
  b = c2 / d, b < 1 && (b = 1), c2 = 1;
  while (c2 <= d) {
    var h, g = o(a.model, c2);
    g > 0 && !_(a, c2) && (h = (c2 - 1 | 0) * b, canvasFillRect(e, 2, h, Sb(g), b, f)), c2 = c2 + 1 | 0;
  }
}
function Mb(a, d, c2) {
  var b = emptyBuf(), e = a.model, f = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
  u(f, d, 1, d, c2), c2 = e.decorations, e = [], na(c2, c2.root, f, e), c2 = 0;
  while (c2 < e.length) b = b + dd + e[c2].className, c2++;
  e = qa(ma(a.model.uri), 0), a = 0;
  while (a < e.length) c2 = e[a], c2[3] == d && (b = b + dd, b = b + Eb(c2[2])), a++;
  return b;
}
function Nb(a, b) {
  var d = [];
  a = hostCall(a, "tokenize", b, void 0, void 0), a && a.tokens && (a = a.tokens);
  var e = jsArrayLen(a), c2 = 0;
  while (c2 < e) b = jsArrayAt(a, c2), b = [0, ""], b[0] = jsPropInt(b, "offset", 0), b[1] = jsPropString(b, xd, jsPropString(b, "scopes", "mtk")), d.push(b), c2++;
  return d;
}
function Ob(a, d, b) {
  return !a.showLineNumbers ? emptyBuf() : '<div class="line-number" data-line="' + d.toString(10) + ld + a.lineHeight.toString(10) + 'px"><span class="folding" data-fold="' + d.toString(10) + Ed + b + "</span> " + d.toString(10) + Rc;
}
function Pb(a, d) {
  a = aa(a, d);
  return !a ? emptyBuf() : a.collapsed ? "\u25B6" : "\u25BC";
}
var Qb;
var j;
(function() {
  var a = /* @__PURE__ */ (function() {
    function b2(a2) {
      var c3 = a2.length - 1;
      while (c3 >= 0) {
        if ("." == a2.charAt(c3)) return r(a2, 0, c3);
        c3--;
      }
      return Ic;
    }
    function c2(a2, c3) {
      while (c3.length > 0) {
        var d2 = a2.stateNames.indexOf(c3);
        if (d2 >= 0) return a2.stateRules[d2];
        c3 = b2(c3);
      }
      c3 = a2.stateNames.indexOf(Bc);
      return c3 >= 0 ? a2.stateRules[c3] : [];
    }
    function d(a2, b3) {
      return 0 == b3.length ? b3 : b3.indexOf(".") >= 0 || 0 == a2.tokenPostfix.length ? b3 : b3 + a2.tokenPostfix;
    }
    return function(a2, b3, e) {
      var g, n2, k2, p2, j2, o2, m2 = [], i2 = 0, h = true, l2 = 0, f = -1;
      while ((h || i2 < e.length) && l2 < 1e4) {
        l2 = l2 + 1 | 0;
        if (!h && i2 == f) {
          f = i2 + 1 | 0;
          if (f >= e.length) break;
        } else {
          f = i2;
        }
        0 == b3[0].length && b3[0].push(Bc);
        for (n2 = b3[0][b3[0].length - 1] || "", k2 = c2(a2, n2), p2 = r(e, f, e.length), j2 = null, g = Ic, i2 = 0; i2 < k2.length; i2++) {
          h = k2[i2];
          if (h.lineStart && 0 != f) continue;
          if (o2 = h.pattern.exec(p2)) {
            g = o2[0] + "", j2 = h;
            break;
          }
        }
        i2 = a2.defaultToken;
        k2 = Ic;
        if (j2) {
          h = j2.kind, i2 = j2.token, k2 = j2.next;
        } else {
          if (f < e.length) {
            g = e.charAt(f);
          } else {
            break;
          }
          h = 0;
        }
        if (0 == g.length) {
          if (f < e.length) {
            g = e.charAt(f), i2 = a2.defaultToken;
          } else {
            break;
          }
          h = 0;
        }
        4 == h && (i2 = a2.keywords.indexOf(g) >= 0 ? _c : Kc);
        6 == h ? i2 = f : "@rematch" != i2 ? (j2 = d(a2, i2), j2.length > 0 && (i2 = [0, ""], i2[0] = f, i2[1] = j2, m2.push(i2)), i2 = f + g.length | 0) : i2 = f, 1 == h ? b3[0].length < a2.maxStack && b3[0].push(k2) : 2 == h ? b3[0].length > 1 && b3[0].splice(b3[0].length - 1, 1) : 3 == h ? b3[0][b3[0].length - 1] = k2 : 5 == h ? b3[0] = [Bc] : 7 == h && b3[0].length < a2.maxStack && b3[0].push(n2);
        if (0 == i2 && 0 == g.length && 0 == h) break;
        h = false;
      }
      return m2;
    };
  })();
  function b(a2) {
    a2 = Q(a2);
    return !a2 ? null : a2.lexer;
  }
  Qb = function(b2, c2, d, e) {
    var g = [];
    return b2 && b2.tokensProvider ? Nb(b2.tokensProvider, e) : c2 ? a(c2, d, e) : g;
  };
  j = function(c2) {
    R(c2), ra(c2);
    var d = Va(c2), h = Ib(c2), l2 = Q(c2.languageId), g = b(c2.languageId), j2 = [[]];
    j2[0] = [Bc];
    var f, i2, e = 1;
    while (e < d && g) a(g, j2, k(c2.model, e)), e = e + 1 | 0;
    e = emptyBuf(), f = emptyBuf();
    while (d <= h) {
      if (_(c2, d)) {
        d = d + 1 | 0;
        continue;
      }
      i2 = k(c2.model, d);
      e += Ob(c2, d, Pb(c2, d)), i2 = '<div class="view-line' + Mb(c2, d, i2.length + 1 | 0) + '" data-line="' + d.toString(10) + ld + c2.lineHeight.toString(10) + 'px">' + Kb(i2, Qb(l2, g, j2, i2)) + Rc, f += i2, d = d + 1 | 0;
    }
    setInnerHTML(c2.margin, e);
    setInnerHTML(c2.linesHost, f), i2 = q(c2.model), d = 0, e = 1;
    while (e <= i2) _(c2, e) || (d = d + 1 | 0), e = e + 1 | 0;
    setStyle(c2.linesHost, Sc, ((d * c2.lineHeight | 0) + (c2.height / 2 | 0) | 0).toString(10) + Xc), Lb(c2), Rb(c2), sa(c2);
  };
})();
function Ya(a, d, b, c2) {
  d = (d - 1 | 0) * a.lineHeight | 0;
  let e = (b - 1 | 0) * a.charWidth | 0;
  return '<div class="selected-text" style="position:absolute;top:' + d.toString(10) + "px;left:" + e.toString(10) + "px;width:" + Za(1, (c2 - b | 0) * a.charWidth | 0).toString(10) + "px;height:" + a.lineHeight.toString(10) + 'px;background:rgba(38,79,120,0.45)"></div>';
}
function Rb(a) {
  var d, e, b = a.selection, c2 = emptyBuf();
  if (b.startLineNumber != b.endLineNumber || b.startColumn != b.endColumn) {
    d = b.startLineNumber;
    while (d <= b.endLineNumber) e = o(a.model, d) + 1 | 0, d == b.endLineNumber && (e = b.endColumn), c2 += Ya(a, d, d == d ? b.startColumn : 1, e), d = d + 1 | 0;
  }
  d = 0;
  while (d < a.extraCursors.length) b = a.extraCursors[d], c2 += Ya(a, b.startLineNumber, b.startColumn, b.endColumn), d++;
  setInnerHTML(a.selectionHost, c2);
}
function sa(a) {
  let c2 = (a.selection.positionLineNumber - 1 | 0) * a.lineHeight | 0, d = (a.selection.positionColumn - 1 | 0) * a.charWidth | 0;
  setStyle(a.textarea, ed, c2.toString(10) + Xc), setStyle(a.textarea, gd, (d + 56 | 0).toString(10) + Xc), setStyle(a.cursorEl, ed, c2.toString(10) + Xc), setStyle(a.cursorEl, gd, d.toString(10) + Xc), inputSetValue(a.textarea, Ic);
}
function ca(a, d, b) {
  var e = d - rectLeft(a.scrollable) | 0;
  d = (((b - rectTop(a.scrollable) | 0) + a.scrollTop | 0) / a.lineHeight | 0) + 1 | 0, d < 1 && (d = 1), b = q(a.model), d > b || (b = d), b = $(a, b), d = (e / a.charWidth | 0) + 1 | 0, a = o(a.model, b) + 1 | 0, d < 1 && (d = 1), d > a || (a = d), d = { lineNumber: 0, column: 0 }, d.lineNumber = b, d.column = a;
  return d;
}
function ta(a, b) {
  b = (b - 1 | 0) * a.lineHeight | 0;
  var c2 = a.scrollTop + a.height | 0;
  (b < a.scrollTop || (b + a.lineHeight | 0) > c2) && (a.scrollTop = Za(0, b - (a.height / 3 | 0) | 0));
}
function Sb(a) {
  return a < 60 ? a : 60;
}
function Za(a, b) {
  return a > b ? a : b;
}
function Tb(a, b) {
  let c2 = { root: null, overflow: null, margin: null, scrollable: null, linesHost: null, textarea: null, minimapCanvas: null, widgetsHost: null, cursorEl: null, selectionHost: null, findWidget: null, findInput: null, replaceInput: null, suggestWidget: null, hoverWidget: null, contextWidget: null, gotoWidget: null, gotoInput: null, renameWidget: null, renameInput: null, paramWidget: null, stickyWidget: null, model: null, selection: null, extraCursors: [], scrollTop: 0, lineHeight: 0, charWidth: 0, width: 0, height: 0, showLineNumbers: false, showMinimap: false, theme: "", languageId: "", readOnly: false, fontSize: 0, tabSize: 0, insertSpaces: false, wordWrap: false, mouseSelecting: false, mouseAnchorLine: 0, mouseAnchorColumn: 0, folds: [], collapsed: [], findOpen: false, findQuery: "", replaceQuery: "", findIndex: 0, suggestOpen: false, suggestItems: [], suggestIndex: 0, hoverOpen: false, contextOpen: false, gotoOpen: false, renameOpen: false, renameWord: "", modelFacade: null };
  Fb(c2, a, b);
  return c2;
}
function O(b) {
  var a = q(b.model), d = b.selection.positionLineNumber;
  d < 1 && (d = 1), d > a && (d = a);
  var f = o(b.model, d) + 1 | 0, c2 = b.selection.positionColumn;
  c2 < 1 && (c2 = 1), c2 > f || (f = c2), c2 = b.selection.selectionStartLineNumber;
  var e = b.selection.selectionStartColumn;
  c2 < 1 && (c2 = 1), c2 > a || (a = c2), c2 = o(b.model, a) + 1 | 0, e < 1 && (e = 1), e > c2 || (c2 = e), e = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 }, C(e, a, c2, d, f), b.selection = e;
}
function m(b, d, a) {
  let c2 = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 };
  C(c2, d, a, d, a), b.selection = c2, O(b), ta(b, b.selection.positionLineNumber), sa(b);
}
function n(b, a, c2, d, e) {
  let f = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 };
  C(f, a, c2, d, e), b.selection = f, O(b), ta(b, b.selection.positionLineNumber), sa(b);
}
function x(b, e) {
  let a = b.selection, c2 = a.startLineNumber, d = a.startColumn;
  l(b.model, [p(c2, d, a.endLineNumber, a.endColumn, e)], true), e = hostGetPositionAt(b.model.buffer, hostGetOffsetAt(b.model.buffer, c2, d, i) + e.length | 0, i), m(b, e.lineNumber, e.column);
}
function _a(a) {
  var b = emptyBuf(), c2 = 0;
  while (c2 < a.length) c2 && (b = b + Ac), b += a[c2] || "", c2++;
  return b;
}
function Ub(a) {
  return "(" == a ? ")" : "[" == a ? "]" : "{" == a ? "}" : '"' == a || "'" == a ? a : Ic;
}
function S(b, e) {
  if (b.readOnly || 0 == e.length) return;
  if (1 == e.length) {
    var a = Ub(e);
    if (a.length > 0) {
      e += a, x(b, e), E(b, e), da(b, false), j(b);
      return;
    }
  }
  x(b, e);
  E(b, e), j(b);
}
function E(b, e) {
  var c2 = b.extraCursors.length;
  if (0 == c2) return;
  c2--;
  while (c2 >= 0) {
    var a = b.extraCursors[c2], d = a.startLineNumber, f = a.startColumn, g = b.model;
    l(g, [p(d, f, a.endLineNumber, a.endColumn, e)], true), d = hostGetPositionAt(g.buffer, hostGetOffsetAt(g.buffer, d, f, i) + e.length | 0, i), a = d.lineNumber, d = d.column, f = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 }, C(f, a, d, a, d), b.extraCursors[c2] = f, c2--;
  }
}
function Vb(b) {
  var c2 = b.extraCursors.length;
  if (0 == c2) return;
  c2--;
  while (c2 >= 0) {
    var d = b.extraCursors[c2], f = d.positionLineNumber, a = d.positionColumn;
    if (1 == f && 1 == a) {
      c2--;
      continue;
    }
    var e = hostGetPositionAt(b.model.buffer, hostGetOffsetAt(b.model.buffer, f, a, i) - 1 | 0, i);
    d = e.lineNumber, e = e.column, l(b.model, [F(d, e, f, a)], true), f = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 }, C(f, d, e, d, e), b.extraCursors[c2] = f, c2--;
  }
}
function Wb(b) {
  var c2 = b.extraCursors.length;
  if (0 == c2) return;
  c2--;
  while (c2 >= 0) {
    var d = b.extraCursors[c2], a = d.positionLineNumber;
    d = d.positionColumn;
    var e = hostGetOffsetAt(b.model.buffer, a, d, i);
    if (e >= b.model.buffer.length) {
      c2--;
      continue;
    }
    e = hostGetPositionAt(b.model.buffer, e + 1 | 0, i);
    l(b.model, [F(a, d, e.lineNumber, e.column)], true), e = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 }, C(e, a, d, a, d), b.extraCursors[c2] = e, c2--;
  }
}
function da(b, a) {
  var c2 = hostGetOffsetAt(b.model.buffer, b.selection.positionLineNumber, b.selection.positionColumn, i);
  c2 > 0 && (c2 = c2 - 1 | 0), c2 = hostGetPositionAt(b.model.buffer, c2, i), a ? n(b, b.selection.selectionStartLineNumber, b.selection.selectionStartColumn, c2.lineNumber, c2.column) : m(b, c2.lineNumber, c2.column);
}
function Xb(b) {
  var a = b.selection.endLineNumber;
  a < q(b.model) && (a = a + 1 | 0);
  var c2 = a == b.selection.endLineNumber ? o(b.model, a) + 1 | 0 : 1;
  n(b, b.selection.startLineNumber, 1, a, c2);
}
function L(b) {
  let a = b.selection, c2 = a.startLineNumber, d = a.startColumn, f = a.endLineNumber;
  let e = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
  u(e, c2, d, f, a.endColumn);
  return La(b.model.buffer, e);
}
function ua(b) {
  var e = L(b);
  0 == e.length && (e = k(b.model, b.selection.positionLineNumber) + Ac), clipboardWrite(e);
}
function va(b) {
  ua(b);
  if (b.readOnly) return;
  var d;
  b.selection.startLineNumber == b.selection.endLineNumber && b.selection.startColumn == b.selection.endColumn ? (d = b.selection.positionLineNumber, d < q(b.model) ? l(b.model, [p(d, 1, d + 1 | 0, 1, Ic)], true) : l(b.model, [p(d, 1, d, o(b.model, d) + 1 | 0, Ic)], true), m(b, d, 1)) : x(b, Ic), j(b);
}
function wa(b, a) {
  if (b.readOnly) return;
  0 == a.length && (a = clipboardRead());
  if (0 == a.length) return;
  x(b, a), E(b, a), j(b);
}
function Yb(b) {
  if (b.readOnly) return;
  var a = b.selection.startLineNumber, h = b.selection.endLineNumber;
  if (h < q(b.model)) {
    l(b.model, [p(a, 1, h + 1 | 0, 1, Ic)], true);
  } else {
    if (a > 1) {
      c = a - 1 | 0;
      var d = o(b.model, c) + 1 | 0, e = b.model;
      l(e, [F(c, d, h, o(e, h) + 1 | 0)], true);
    } else {
      l(b.model, [p(1, 1, h, o(b.model, h) + 1 | 0, Ic)], true);
    }
  }
  m(b, a, 1);
  j(b);
}
function $a(b, a) {
  if (b.readOnly) return;
  var c2 = b.selection.startLineNumber, h = b.selection.endLineNumber, e = [], d = c2;
  while (c2 <= h) e.push(k(b.model, c2)), d = d + 1 | 0;
  d = _a(e), a < 0 ? (a = b.model, l(a, [p(c2, 1, c2, 1, d + Ac)], true)) : (a = o(b.model, h) + 1 | 0, e = b.model, l(e, [p(h, a, h, a, Ac + d)], true), a = (h - c2 | 0) + 1 | 0, c2 = c2 + a | 0, a = h + a | 0, n(b, c2, 1, a, o(e, a) + 1 | 0)), j(b);
}
function ab(b, d) {
  var a = b.selection;
  d = a.positionLineNumber + d | 0;
  var c2;
  if (d < 1 || d > q(b.model)) return;
  c2 = o(b.model, d) + 1 | 0, a.positionColumn > c2 && (a = c2), c2 = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0, selectionStartLineNumber: 0, selectionStartColumn: 0, positionLineNumber: 0, positionColumn: 0 }, C(c2, d, a, d, a), b.extraCursors.push(c2);
}
function Zb(b) {
  if (b.readOnly) return;
  var a, h, c2, e = q(b.model), d = 1;
  while (d <= e) {
    a = k(b.model, d), h = a.length;
    while (h > 0) {
      c2 = a.charAt(h - 1);
      if (c2 != dd && c2 != Dd) break;
      h--;
    }
    h < h && (c2 = b.model, l(c2, [F(d, h + 1 | 0, d, h + 1 | 0)], true));
    d = d + 1 | 0;
  }
  j(b);
}
function _b(b) {
  if (b.readOnly) return;
  var d = b.selection.endLineNumber, a = o(b.model, d) + 1 | 0, c2 = b.model;
  l(c2, [p(d, a, d, a, Ac)], true), m(b, d + 1 | 0, 1), j(b);
}
function $b(b) {
  if (b.readOnly) return;
  var d = b.selection.startLineNumber, a = b.model;
  l(a, [p(d, 1, d, 1, Ac)], true), m(b, d, 1), j(b);
}
function ac(b) {
  if (b.readOnly) return;
  var d = b.selection.startLineNumber;
  if (d >= q(b.model)) return;
  var a = k(b.model, d), c2 = k(b.model, d + 1 | 0), e = r(c2, B(c2), c2.length);
  e = a.length > 0 && e.length > 0 ? a + dd + e : a + e, l(b.model, [p(d, 1, d + 1 | 0, c2.length + 1 | 0, e)], true), m(b, d, a.length + 1 | 0), j(b);
}
function bc(b) {
  let a = s(k(b.model, b.selection.positionLineNumber), b.selection.positionColumn);
  n(b, b.selection.positionLineNumber, a[1], b.selection.positionLineNumber, a[2]);
}
var cc;
var dc;
(function() {
  function a(b2) {
    if (b2.readOnly) return;
    var c3, d2, e2, a2 = b2.selection;
    if (a2.startLineNumber != a2.endLineNumber || a2.startColumn != a2.endColumn) {
      x(b2, Ic), E(b2, Ic), j(b2);
      return;
    }
    if (1 == a2.positionLineNumber && 1 == a2.positionColumn) return;
    c3 = a2.positionLineNumber, a2 = a2.positionColumn, e2 = hostGetPositionAt(b2.model.buffer, hostGetOffsetAt(b2.model.buffer, c3, a2, i) - 1 | 0, i), d2 = e2.lineNumber, e2 = e2.column, l(b2.model, [F(d2, e2, c3, a2)], true), m(b2, d2, e2), Vb(b2), j(b2);
  }
  function b(b2) {
    if (b2.readOnly) return;
    var c3, d2, a2 = b2.selection;
    if (a2.startLineNumber != a2.endLineNumber || a2.startColumn != a2.endColumn) {
      x(b2, Ic), E(b2, Ic), j(b2);
      return;
    }
    c3 = a2.positionLineNumber;
    a2 = a2.positionColumn, d2 = hostGetOffsetAt(b2.model.buffer, c3, a2, i);
    if (d2 >= b2.model.buffer.length) return;
    d2 = hostGetPositionAt(b2.model.buffer, d2 + 1 | 0, i), l(b2.model, [F(c3, a2, d2.lineNumber, d2.column)], true), Wb(b2), j(b2);
  }
  function c2(b2, a2) {
    var c3 = hostGetOffsetAt(b2.model.buffer, b2.selection.positionLineNumber, b2.selection.positionColumn, i);
    c3 < b2.model.buffer.length && (c3 = c3 + 1 | 0), c3 = hostGetPositionAt(b2.model.buffer, c3, i), a2 ? n(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, c3.lineNumber, c3.column) : m(b2, c3.lineNumber, c3.column);
  }
  function d(b2, a2) {
    var d2 = b2.selection.positionLineNumber - 1 | 0;
    d2 < 1 && (d2 = 1);
    var c3 = $(b2, d2);
    d2 = b2.selection.positionColumn;
    var e2 = o(b2.model, c3) + 1 | 0;
    d2 > e2 && (d2 = e2), a2 ? n(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, c3, d2) : m(b2, c3, d2);
  }
  function e(b2, a2) {
    var d2 = b2.selection.positionLineNumber + 1 | 0, c3 = q(b2.model);
    d2 > c3 && (d2 = c3), c3 = $(b2, d2), d2 = b2.selection.positionColumn;
    var e2 = o(b2.model, c3) + 1 | 0;
    d2 > e2 && (d2 = e2), a2 ? n(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, c3, d2) : m(b2, c3, d2);
  }
  function f(b2, a2) {
    var d2 = b2.selection.positionLineNumber, e2 = k(b2.model, d2), c3 = B(e2);
    c3 = b2.selection.positionColumn != (c3 + 1 | 0) && c3 < e2.length ? c3 + 1 | 0 : 1, a2 ? n(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, d2, c3) : m(b2, d2, c3);
  }
  function g(b2, a2) {
    var d2 = b2.selection.positionLineNumber, c3 = o(b2.model, d2) + 1 | 0;
    a2 ? n(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, d2, c3) : m(b2, d2, c3);
  }
  function h(b2, a2) {
    a2 ? n(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, 1, 1) : m(b2, 1, 1);
  }
  function t2(b2, a2) {
    var c3 = q(b2.model), d2 = o(b2.model, c3) + 1 | 0;
    a2 ? n(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, c3, d2) : m(b2, c3, d2);
  }
  function u2(b2, a2) {
    var d2 = b2.height / b2.lineHeight | 0;
    d2 < 1 && (d2 = 1), d2 = b2.selection.positionLineNumber - d2 | 0, d2 < 1 && (d2 = 1), a2 ? n(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, d2, b2.selection.positionColumn) : m(b2, d2, b2.selection.positionColumn);
  }
  function v2(b2, a2) {
    var d2 = b2.height / b2.lineHeight | 0;
    d2 < 1 && (d2 = 1), d2 = b2.selection.positionLineNumber + d2 | 0;
    var c3 = q(b2.model);
    d2 > c3 && (d2 = c3), a2 ? n(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, d2, b2.selection.positionColumn) : m(b2, d2, b2.selection.positionColumn);
  }
  function w2(b2, a2) {
    var c3 = hostGetOffsetAt(b2.model.buffer, b2.selection.positionLineNumber, b2.selection.positionColumn, i);
    c3 > 0 && (c3 = c3 - 1 | 0), c3 = hostGetPositionAt(b2.model.buffer, c3, i);
    var d2 = s(k(b2.model, c3.lineNumber), c3.column);
    a2 ? n(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, c3.lineNumber, d2[1]) : m(b2, c3.lineNumber, d2[1]);
  }
  function y2(b2, a2) {
    var c3 = s(k(b2.model, b2.selection.positionLineNumber), b2.selection.positionColumn)[2];
    if (c3 == b2.selection.positionColumn) {
      var d2 = hostGetOffsetAt(b2.model.buffer, b2.selection.positionLineNumber, b2.selection.positionColumn, i);
      d2 < b2.model.buffer.length && (c3 = hostGetPositionAt(b2.model.buffer, d2 + 1 | 0, i), c3 = s(k(b2.model, c3.lineNumber), c3.column)[2]);
    }
    a2 ? n(b2, b2.selection.selectionStartLineNumber, b2.selection.selectionStartColumn, b2.selection.positionLineNumber, c3) : m(b2, b2.selection.positionLineNumber, c3);
  }
  function z2(b2) {
    if (b2.readOnly) return;
    var c3, a2 = b2.selection;
    if (a2.startLineNumber != a2.endLineNumber || a2.startColumn != a2.endColumn) {
      x(b2, Ic), E(b2, Ic), j(b2);
      return;
    }
    c3 = s(k(b2.model, a2.positionLineNumber), a2.positionColumn)[1];
    c3 == a2.positionColumn && a2.positionColumn > 1 && (c3 = a2.positionColumn - 1 | 0), l(b2.model, [F(a2.positionLineNumber, c3, a2.positionLineNumber, a2.positionColumn)], true), m(b2, a2.positionLineNumber, c3), j(b2);
  }
  function A2(b2) {
    if (b2.readOnly) return;
    var c3, d2, a2 = b2.selection;
    if (a2.startLineNumber != a2.endLineNumber || a2.startColumn != a2.endColumn) {
      x(b2, Ic), E(b2, Ic), j(b2);
      return;
    }
    c3 = s(k(b2.model, a2.positionLineNumber), a2.positionColumn)[2];
    c3 == a2.positionColumn && (c3 = a2.positionColumn + 1 | 0, d2 = o(b2.model, a2.positionLineNumber) + 1 | 0, c3 > d2 && (c3 = d2)), l(b2.model, [F(a2.positionLineNumber, a2.positionColumn, a2.positionLineNumber, c3)], true), j(b2);
  }
  function C2(b2, a2) {
    if (b2.readOnly) return;
    var e2 = Dd;
    b2.insertSpaces && (e2 = mb(b2.tabSize));
    var d2, g2, c3, f2 = b2.selection;
    if (f2.startLineNumber != f2.endLineNumber || a2) {
      d2 = f2.startLineNumber;
      while (d2 <= f2.endLineNumber) {
        g2 = k(b2.model, d2);
        if (a2) {
          if (g2.length > 0 && g2.charAt(0) == Dd) {
            c3 = 1;
          } else {
            c3 = 0;
            while (c3 < b2.tabSize && c3 < g2.length && g2.charAt(c3) == dd) c3 = c3 + 1 | 0;
          }
          c3 > 0 && (g2 = b2.model, l(g2, [p(d2, 1, d2, c3 + 1 | 0, Ic)], true));
        } else {
          l(b2.model, [p(d2, 1, d2, 1, e2)], true);
        }
        d2 = d2 + 1 | 0;
      }
      j(b2);
      return;
    }
    S(b2, e2);
  }
  function D2(b2) {
    if (b2.readOnly) return;
    var c3 = k(b2.model, b2.selection.positionLineNumber), a2 = r(c3, 0, B(c3)), d2 = emptyBuf();
    b2.selection.positionColumn > 1 && (d2 = r(c3, b2.selection.positionColumn - 2 | 0, b2.selection.positionColumn - 1 | 0)), "{" == d2 && (a2 = a2 + "  "), a2 = Ac + a2, x(b2, a2), E(b2, a2), j(b2);
  }
  function G2(b2, a2) {
    if (b2.readOnly) return;
    var c3 = b2.selection.startLineNumber, h2 = b2.selection.endLineNumber, d2 = c3 + a2 | 0, e2 = q(b2.model);
    if (d2 < 1 || (h2 + a2 | 0) > e2) return;
    e2 = [];
    while (c3 <= h2) e2.push(k(b2.model, c3)), d2 = d2 + 1 | 0;
    d2 = _a(e2), a2 < 0 ? (a2 = c3 - 1 | 0, c3 = k(b2.model, a2), e2 = b2.model, l(e2, [p(a2, 1, h2, o(e2, h2) + 1 | 0, d2 + Ac + c3)], true), h2 = h2 - 1 | 0, n(b2, a2, 1, h2, o(e2, h2) + 1 | 0)) : (a2 = h2 + 1 | 0, h2 = k(b2.model, a2), e2 = b2.model, l(e2, [p(c3, 1, a2, o(e2, a2) + 1 | 0, h2 + Ac + d2)], true), h2 = c3 + 1 | 0, n(b2, h2, 1, a2, o(e2, a2) + 1 | 0)), j(b2);
  }
  cc = function(i2, k2, l2) {
    if (k2 == xd || "compositionType" == k2) {
      S(i2, l2);
      return true;
    }
    if ("paste" == k2 || "editor.action.clipboardPasteAction" == k2) {
      wa(i2, l2);
      return true;
    }
    if ("cut" == k2 || "editor.action.clipboardCutAction" == k2) {
      va(i2);
      return true;
    }
    if ("copy" == k2 || "editor.action.clipboardCopyAction" == k2) {
      ua(i2);
      return true;
    }
    if (k2 == Ld || k2 == Ld) {
      a(i2);
      return true;
    }
    if ("deleteRight" == k2) {
      b(i2);
      return true;
    }
    if ("deleteWordLeft" == k2 || "deleteWordStartLeft" == k2) {
      z2(i2);
      return true;
    }
    if ("deleteWordRight" == k2 || "deleteWordEndRight" == k2) {
      A2(i2);
      return true;
    }
    if ("undo" == k2) {
      Pa(i2.model), O(i2), j(i2);
      return true;
    }
    if ("redo" == k2) {
      Qa(i2.model), O(i2), j(i2);
      return true;
    }
    if ("tab" == k2 || "editor.action.indentLines" == k2) {
      C2(i2, false);
      return true;
    }
    if ("outdent" == k2 || "editor.action.outdentLines" == k2) {
      C2(i2, true);
      return true;
    }
    if ("cursorLeft" == k2) {
      da(i2, false);
      return true;
    }
    if ("cursorRight" == k2) {
      c2(i2, false);
      return true;
    }
    if ("cursorUp" == k2) {
      d(i2, false);
      return true;
    }
    if ("cursorDown" == k2) {
      e(i2, false);
      return true;
    }
    if ("cursorHome" == k2 || "cursorLineStart" == k2) {
      f(i2, false);
      return true;
    }
    if ("cursorEnd" == k2 || "cursorLineEnd" == k2) {
      g(i2, false);
      return true;
    }
    if ("cursorTop" == k2) {
      h(i2, false);
      return true;
    }
    if ("cursorBottom" == k2) {
      t2(i2, false);
      return true;
    }
    if ("cursorPageUp" == k2) {
      u2(i2, false);
      return true;
    }
    if ("cursorPageDown" == k2) {
      v2(i2, false);
      return true;
    }
    if ("cursorWordLeft" == k2 || "cursorWordStartLeft" == k2) {
      w2(i2, false);
      return true;
    }
    if ("cursorWordRight" == k2 || "cursorWordEndRight" == k2) {
      y2(i2, false);
      return true;
    }
    if ("cursorLeftSelect" == k2) {
      da(i2, true);
      return true;
    }
    if ("cursorRightSelect" == k2) {
      c2(i2, true);
      return true;
    }
    if ("cursorUpSelect" == k2) {
      d(i2, true);
      return true;
    }
    if ("cursorDownSelect" == k2) {
      e(i2, true);
      return true;
    }
    if ("cursorHomeSelect" == k2) {
      f(i2, true);
      return true;
    }
    if ("cursorEndSelect" == k2) {
      g(i2, true);
      return true;
    }
    if ("cursorWordLeftSelect" == k2) {
      w2(i2, true);
      return true;
    }
    if ("cursorWordRightSelect" == k2) {
      y2(i2, true);
      return true;
    }
    if ("selectAll" == k2) {
      k2 = q(i2.model), n(i2, 1, 1, k2, o(i2.model, k2) + 1 | 0);
      return true;
    }
    if (k2 == kd) {
      Xb(i2);
      return true;
    }
    if ("enter" == k2) {
      D2(i2);
      return true;
    }
    if ("editor.action.deleteLines" == k2) {
      Yb(i2);
      return true;
    }
    if ("editor.action.moveLinesUpAction" == k2) {
      G2(i2, -1);
      return true;
    }
    if ("editor.action.moveLinesDownAction" == k2) {
      G2(i2, 1);
      return true;
    }
    if ("editor.action.copyLinesUpAction" == k2) {
      $a(i2, -1);
      return true;
    }
    if ("editor.action.copyLinesDownAction" == k2) {
      $a(i2, 1);
      return true;
    }
    if ("editor.action.insertCursorAbove" == k2) {
      ab(i2, -1);
      return true;
    }
    if ("editor.action.insertCursorBelow" == k2) {
      ab(i2, 1);
      return true;
    }
    if ("editor.action.trimTrailingWhitespace" == k2) {
      Zb(i2);
      return true;
    }
    if ("editor.action.insertLineAfter" == k2) {
      _b(i2);
      return true;
    }
    if ("editor.action.insertLineBefore" == k2) {
      $b(i2);
      return true;
    }
    if ("editor.action.joinLines" == k2) {
      ac(i2);
      return true;
    }
    if (k2 == qd) {
      bc(i2);
      return true;
    }
    if ("editor.action.fontZoomIn" == k2) {
      i2.fontSize = i2.fontSize + 1 | 0, i2.lineHeight = i2.fontSize + 5 | 0, j(i2);
      return true;
    }
    if ("editor.action.fontZoomOut" == k2) {
      i2.fontSize > 8 && (i2.fontSize = i2.fontSize - 1 | 0, i2.lineHeight = i2.fontSize + 5 | 0, j(i2));
      return true;
    }
    if ("editor.action.fontZoomReset" == k2) {
      i2.fontSize = 14, i2.lineHeight = 19, j(i2);
      return true;
    }
    return false;
  };
  dc = function(i2, k2) {
    var l2 = eventKey(k2), p2 = eventCtrlKey(k2), m2 = eventShiftKey(k2), H2 = eventAltKey(k2);
    if (p2 && !m2 && ("z" == l2 || "Z" == l2)) {
      preventDefault(k2), Pa(i2.model), O(i2), j(i2);
      return;
    }
    if (p2 && ("y" == l2 || "Y" == l2 || m2 && ("z" == l2 || "Z" == l2))) {
      preventDefault(k2), Qa(i2.model), O(i2), j(i2);
      return;
    }
    if (p2 && ("a" == l2 || "A" == l2)) {
      preventDefault(k2), k2 = q(i2.model), n(i2, 1, 1, k2, o(i2.model, k2) + 1 | 0);
      return;
    }
    if (p2 && ("c" == l2 || "C" == l2)) {
      preventDefault(k2), ua(i2);
      return;
    }
    if (p2 && ("x" == l2 || "X" == l2)) {
      preventDefault(k2), va(i2);
      return;
    }
    if (p2 && ("v" == l2 || "V" == l2)) {
      preventDefault(k2), wa(i2, Ic);
      return;
    }
    if ("Backspace" == l2) {
      preventDefault(k2), p2 ? z2(i2) : a(i2);
      return;
    }
    if ("Delete" == l2) {
      preventDefault(k2), p2 ? A2(i2) : b(i2);
      return;
    }
    if (l2 == Yc) {
      preventDefault(k2), D2(i2);
      return;
    }
    if ("Tab" == l2) {
      preventDefault(k2), C2(i2, m2);
      return;
    }
    if ("Home" == l2) {
      preventDefault(k2), p2 ? h(i2, m2) : f(i2, m2);
      return;
    }
    if ("End" == l2) {
      preventDefault(k2), p2 ? t2(i2, m2) : g(i2, m2);
      return;
    }
    if ("PageUp" == l2) {
      preventDefault(k2), u2(i2, m2);
      return;
    }
    if ("PageDown" == l2) {
      preventDefault(k2), v2(i2, m2);
      return;
    }
    if ("ArrowLeft" == l2) {
      preventDefault(k2), p2 ? w2(i2, m2) : da(i2, m2);
      return;
    }
    if ("ArrowRight" == l2) {
      preventDefault(k2), p2 ? y2(i2, m2) : c2(i2, m2);
      return;
    }
    if ("ArrowUp" == l2) {
      preventDefault(k2), H2 ? G2(i2, -1) : d(i2, m2);
      return;
    }
    if (l2 == Od) {
      preventDefault(k2), H2 ? G2(i2, 1) : e(i2, m2);
      return;
    }
  };
})();
function ec(b, a) {
  preventDefault(a), wa(b, clipboardReadEvent(a));
}
function bb(e, a) {
  preventDefault(a), e = L(e), clipboardWriteEvent(a, e), clipboardWrite(e);
}
function fc(b, a) {
  bb(b, a), va(b);
}
function gc(a, b) {
  return "*" == a || a == b || 0 == a.length;
}
function cb(g) {
  if (g) {
    if (g.range) return cb(g.range);
    var a = g.startLineNumber ? +g.startLineNumber | 0 : 1, b = g.startColumn ? +g.startColumn | 0 : 1, f = g.endLineNumber ? +g.endLineNumber | 0 : 1;
    g = g.endColumn ? +g.endColumn | 0 : 1;
  } else {
    a = 1, b = 1, f = 1, g = 1;
  }
  var c2 = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
  u(c2, a, b, f, g);
  return c2;
}
function ea(b) {
  b.findOpen = true, setDisplay(b.findWidget, yc), L(b).length > 0 && (b.findQuery = L(b), inputSetValue(b.findInput, b.findQuery)), focusElement(b.findInput);
}
function xa(b) {
  b.findQuery = inputGetValue(b.findInput);
  return 0 == b.findQuery.length ? [] : J(b.model, b.findQuery, false);
}
function fa(b, a) {
  var c2 = xa(b);
  if (0 == c2.length) return;
  a ? (b.findIndex = b.findIndex - 1 | 0, b.findIndex < 0 && (b.findIndex = c2.length - 1)) : (b.findIndex = b.findIndex + 1 | 0, b.findIndex >= c2.length && (b.findIndex = 0)), a = c2[b.findIndex], n(b, a[0].startLineNumber, a[0].startColumn, a[0].endLineNumber, a[0].endColumn), j(b);
}
var db;
var ya;
var jc;
var kc;
var lc;
var mc;
(function() {
  function a(a2, b2, c3, d2) {
    if (0 == a2.length) return;
    if (c3.length >= 200) return;
    if (d2.indexOf(a2) >= 0) return;
    (0 == b2.length || 0 == a2.indexOf(b2) && a2 != b2) && (b2 = { label: "", insertText: "", kind: 0, detail: "" }, b2.label = a2, b2.insertText = a2, b2.kind = 18, b2.detail = emptyBuf(), c3.push(b2), d2.push(a2));
  }
  function b(d2, b2, e2, f2) {
    var h, j2, g2 = emptyBuf(), i2 = d2.length, c3 = 0;
    while (c3 <= i2) h = emptyBuf(), j2 = c3 < i2 ? oa(d2.charAt(c3)) : false, j2 ? g2 = g2 + h : (a(g2, b2, e2, f2), g2 = emptyBuf()), c3 = c3 + 1 | 0;
  }
  function c2(c3, e2) {
    var h = s(k(c3, e2.lineNumber), e2.column)[0];
    e2 = [];
    var f2 = [], i2 = q(c3), d2 = 1;
    while (d2 <= i2 && e2.length < 200) b(k(c3, d2), h, e2, f2), d2 = d2 + 1 | 0;
    if (c3 = Q(c3.languageId)) {
      d2 = 0;
      while (d2 < c3.lexer.keywords.length) a(c3.lexer.keywords[d2] || "", h, e2, f2), d2++;
    }
    return e2;
  }
  function d(a2, b2) {
    var d2 = [], c3 = 0;
    while (c3 < U.length) U[c3][0] == a2 && gc(U[c3][1], b2) && d2.push(U[c3][2]), c3++;
    return d2;
  }
  function e(a2, b2, c3, d2, e2) {
    return hostCall(a2, b2, c3, d2, e2);
  }
  function f(a2) {
    var e2, b2, h, d2, f2 = [], g2 = jsArrayLen(a2), c3 = 0;
    while (c3 < g2) e2 = jsArrayAt(a2, c3), b2 = jsPropString(e2, Jd, Ic), h = jsPropString(e2, Md, b2), 0 == b2.length && (b2 = jsPropString(e2, Md, Ic)), b2.length > 0 && (d2 = { label: "", insertText: "", kind: 0, detail: "" }, d2.label = b2, d2.insertText = h, d2.kind = 18, d2.detail = emptyBuf(), d2.kind = jsPropInt(e2, "kind", 18), f2.push(d2)), c3++;
    return f2;
  }
  function g(a2) {
    if (!a2) return emptyBuf();
    var c3 = jsPropString(a2, Zc, Ic);
    if (c3.length > 0) return c3;
    var b2 = a2.contents, d2 = jsArrayLen(b2);
    if (0 == d2) {
      return jsArrayLen(a2) > 0 ? jsPropString(jsArrayAt(a2, 0), Zc, jsPropString(jsArrayAt(a2, 0), Jd, Ic)) : a2 + "";
    }
    a2 = emptyBuf();
    c3 = 0;
    while (c3 < d2) {
      var e2 = jsArrayAt(b2, c3);
      e2 = jsPropString(e2, Zc, e2 + ""), a2.length > 0 && (a2 = a2 + Ac), a2 += e2, c3++;
    }
    return a2;
  }
  db = function(b2) {
    var g2 = b2.model, h = b2.selection.positionColumn, a2 = { lineNumber: 0, column: 0 };
    a2.lineNumber = b2.selection.positionLineNumber, a2.column = h, g2 = c2(g2, a2);
    var i2 = d("completion", b2.languageId);
    a2 = 0;
    while (a2 < i2.length) {
      var j2 = f(e(i2[a2], "provideCompletionItems", b2.modelFacade, { __proto__: null, lineNumber: b2.selection.positionLineNumber, column: b2.selection.positionColumn }, void 0));
      h = 0;
      while (h < j2.length) g2.push(j2[h]), h++;
      a2++;
    }
    b2.suggestItems = g2;
    b2.suggestIndex = 0, b2.suggestOpen = g2.length > 0;
    if (!b2.suggestOpen) {
      setDisplay(b2.suggestWidget, xc);
      return;
    }
    h = emptyBuf();
    a2 = 0;
    while (a2 < g2.length && a2 < 12) i2 = Ic, a2 == b2.suggestIndex && (i2 = " background:#04395e;"), h = h + '<div data-suggest="' + a2.toString(10) + '" style="padding:2px 6px;' + i2 + Ed + b2.suggestItems[a2].label + Rc, a2++;
    setInnerHTML(b2.suggestWidget, h), g2 = ((b2.selection.positionColumn - 1 | 0) * b2.charWidth | 0) + 56 | 0, setStyle(b2.suggestWidget, ed, (b2.selection.positionLineNumber * b2.lineHeight | 0).toString(10) + Xc), setStyle(b2.suggestWidget, gd, g2.toString(10) + Xc), setDisplay(b2.suggestWidget, yc);
  };
  ya = function(b2) {
    var a2 = b2.selection.positionColumn, c3 = { lineNumber: 0, column: 0 };
    c3.lineNumber = b2.selection.positionLineNumber, c3.column = a2, a2 = s(k(b2.model, c3.lineNumber), c3.column)[0];
    var i2, h = d("hover", b2.languageId), f2 = 0;
    while (f2 < h.length) i2 = g(e(h[f2], "provideHover", b2.modelFacade, { __proto__: null, lineNumber: c3.lineNumber, column: c3.column }, void 0)), i2.length > 0 && (a2.length > 0 && (a2 = a2 + Ac), a2 = a2 + i2), f2++;
    h = qa(ma(b2.model.uri), 0), f2 = 0;
    while (f2 < h.length) h[f2][3] == c3.lineNumber && (a2.length > 0 && (a2 = a2 + Ac), a2 = a2 + h[f2][1]), f2++;
    b2.hoverOpen = a2.length > 0;
    if (!b2.hoverOpen) {
      setDisplay(b2.hoverWidget, xc);
      return;
    }
    setTextContent(b2.hoverWidget, a2);
    setStyle(b2.hoverWidget, ed, (c3.lineNumber * b2.lineHeight | 0).toString(10) + Xc), setStyle(b2.hoverWidget, gd, (((c3.column - 1 | 0) * b2.charWidth | 0) + 56 | 0).toString(10) + Xc), setDisplay(b2.hoverWidget, yc);
  }, jc = function(b2) {
    var a2 = b2.selection.positionColumn;
    a2 = s(k(b2.model, b2.selection.positionLineNumber), a2)[0] + "(value, options)";
    var c3 = d("signatureHelp", b2.languageId);
    c3.length > 0 && (c3 = g(e(c3[0], "provideSignatureHelp", b2.modelFacade, { __proto__: null, lineNumber: b2.selection.positionLineNumber, column: b2.selection.positionColumn }, void 0)), c3.length > 0 && (a2 = c3)), setTextContent(b2.paramWidget, a2), setDisplay(b2.paramWidget, yc);
  }, kc = function(b2) {
    var a2 = d("definition", b2.languageId);
    if (a2.length > 0) {
      if (a2 = e(a2[0], "provideDefinition", b2.modelFacade, { __proto__: null, lineNumber: b2.selection.positionLineNumber, column: b2.selection.positionColumn }, void 0)) {
        !a2[0] || (a2 = a2[0]), a2 = cb(a2), m(b2, a2.startLineNumber, a2.startColumn), j(b2);
        return;
      }
    }
    a2 = J(b2.model, s(k(b2.model, b2.selection.positionLineNumber), b2.selection.positionColumn)[0], true);
    a2.length > 0 && (m(b2, a2[0][0].startLineNumber, a2[0][0].startColumn), j(b2));
  }, lc = function(b2) {
    var a2 = d("documentFormatting", b2.languageId);
    if (a2.length > 0) {
      a2 = e(a2[0], "provideDocumentFormattingEdits", b2.modelFacade, void 0, void 0);
      var g2, h, c3 = a2 ? 1 : 0;
      if (c3 > 0 && a2[0]) {
        c3 = emptyBuf(), !a2[0].text || (a2 = a2[0], c3 = a2.text + "");
        if (c3.length > 0) {
          a2 = q(b2.model), l(b2.model, [p(1, 1, a2, o(b2.model, a2) + 1 | 0, c3)], true), j(b2);
          return;
        }
      }
    }
    a2 = b2.model;
    c3 = a2.buffer, c3 = Db(A(c3, c3.root)), g2 = c3.replace(/\r\n|\r|\n/g, Ac), h = H(g2), c3 = { buffer: "", lineStarts: [] }, c3.buffer = g2, c3.lineStarts = h, c3 = [c3], g2 = { root: null, buffers: [], lineCnt: 0, length: 0, eol: "", eolLength: 0, eolNormalized: false, lastChangeBufferPos: null, cacheNode: null, cacheNodeStartOffset: 0, cacheNodeStartLineNumber: 0, cacheHasLine: false, cacheValid: false, lastVisitedLineNumber: 0, lastVisitedLineValue: "", posNode: null, posRemainder: 0, posStart: 0, walkLine: 0, walkCol: 0, tmpBuffer: null }, Ea(g2, c3, Ac, true), a2.buffer = g2, a2.versionId = a2.versionId + 1 | 0, w(a2.onDidChangeContent), j(b2);
  }, mc = function(b2) {
    var a2, g2, c3, i2 = d("codeAction", b2.languageId), f2 = emptyBuf(), h = 0;
    while (h < i2.length) {
      a2 = e(i2[h], "provideCodeActions", b2.modelFacade, { __proto__: null, startLineNumber: b2.selection.startLineNumber, startColumn: b2.selection.startColumn, endLineNumber: b2.selection.endLineNumber, endColumn: b2.selection.endColumn }, void 0), g2 = jsArrayLen(a2), a2 && a2.actions && (g2 = jsArrayLen(a2.actions), a2 = a2.actions), c3 = 0;
      while (c3 < g2) f2 += "<div>", f2 = f2 + jsPropString(jsArrayAt(a2, c3), "title", jsPropString(jsArrayAt(a2, c3), Jd, "fix")) + Rc, c3++;
      h++;
    }
    0 == f2.length && (f2 = "<div>No code actions</div>");
    setInnerHTML(b2.hoverWidget, f2), setDisplay(b2.hoverWidget, yc), b2.hoverOpen = true;
  };
})();
function eb(b) {
  var a, c2, e, f, g;
  if (!b.suggestOpen || 0 == b.suggestItems.length) return;
  c2 = b.suggestItems[b.suggestIndex], a = s(k(b.model, b.selection.positionLineNumber), b.selection.positionColumn), e = b.selection.positionLineNumber, f = a[1], g = b.selection.positionLineNumber, l(b.model, [p(e, f, g, a[2], Sa(c2.insertText))], true), b.suggestOpen = false, setDisplay(b.suggestWidget, xc), j(b);
}
function fb(b, a) {
  if (!b.suggestOpen) return;
  b.suggestIndex = b.suggestIndex + a | 0, b.suggestIndex < 0 && (b.suggestIndex = b.suggestItems.length - 1), b.suggestIndex >= b.suggestItems.length && (b.suggestIndex = 0), db(b);
}
function hc(b) {
  var d = inputGetValue(b.gotoInput);
  d = d.length > 0 ? +d | 0 : 1;
  var a = q(b.model);
  d < 1 && (d = 1), d > a || (a = d), m(b, a, 1), b.gotoOpen = false, setDisplay(b.gotoWidget, xc), j(b), focusElement(b.textarea);
}
function ic(b) {
  var d = inputGetValue(b.renameInput);
  if (b.renameWord.length > 0 && d.length > 0) {
    var e = J(b.model, b.renameWord, true), c2 = e.length - 1;
    while (c2 >= 0) {
      var a = e[c2], f = b.model;
      l(f, [p(a[0].startLineNumber, a[0].startColumn, a[0].endLineNumber, a[0].endColumn, d)], true), c2--;
    }
  }
  b.renameOpen = false;
  setDisplay(b.renameWidget, xc), j(b), focusElement(b.textarea);
}
function gb(b, a, c2) {
  b.contextOpen = true, setInnerHTML(b.contextWidget, '<div data-cmd="editor.action.clipboardCutAction">Cut</div><div data-cmd="editor.action.clipboardCopyAction">Copy</div><div data-cmd="editor.action.clipboardPasteAction">Paste</div><div data-cmd="editor.action.commentLine">Toggle Line Comment</div><div data-cmd="editor.action.formatDocument">Format Document</div><div data-cmd="editor.action.rename">Rename Symbol</div><div data-cmd="editor.action.goToDefinition">Go to Definition</div><div data-cmd="editor.action.peekDefinition">Peek References</div>'), setStyle(b.contextWidget, ed, c2.toString(10) + Xc), setStyle(b.contextWidget, gd, a.toString(10) + Xc), setDisplay(b.contextWidget, yc);
}
function M(a, b, c2) {
  if (!a) return c2;
  a = a[b];
  return !a ? c2 : a + "";
}
function nc(a, b) {
  a.view = b;
  let c2 = { listeners: [], disposed: false };
  c2.listeners = [], c2.disposed = false, a.contentEmitter = c2, c2 = { listeners: [], disposed: false }, c2.listeners = [], c2.disposed = false, a.cursorEmitter = c2, a.disposed = false, a.actions = [], a.modelFacade = void 0, b.model.onDidChangeContent.listeners.push(function() {
    w(a.contentEmitter);
  }), oc(a);
}
function oc(a) {
  let b = a.view, d = "keydown", c2 = false;
  b.textarea.addEventListener(d, function(c3) {
    if (b.suggestOpen) {
      var d2 = eventKey(c3);
      if (d2 == Od) {
        preventDefault(c3), fb(b, 1);
        return;
      }
      if ("ArrowUp" == d2) {
        preventDefault(c3), fb(b, -1);
        return;
      }
      if (d2 == Yc || "Tab" == d2) {
        preventDefault(c3), eb(b);
        return;
      }
      if (d2 == zd) {
        preventDefault(c3), b.suggestOpen = false, setDisplay(b.suggestWidget, xc);
        return;
      }
    }
    if (eventCtrlKey(c3) && "f" == eventKey(c3)) {
      preventDefault(c3), ea(b);
      return;
    }
    if (eventCtrlKey(c3) && "h" == eventKey(c3)) {
      preventDefault(c3), ea(b);
      return;
    }
    if (eventCtrlKey(c3) && "g" == eventKey(c3)) {
      preventDefault(c3), v(a, $c, void 0);
      return;
    }
    if (eventCtrlKey(c3) && eventKey(c3) == dd) {
      preventDefault(c3), v(a, Qc, void 0);
      return;
    }
    if (eventCtrlKey(c3) && "/" == eventKey(c3)) {
      preventDefault(c3), v(a, Vc, void 0);
      return;
    }
    if (eventCtrlKey(c3) && ("d" == eventKey(c3) || "D" == eventKey(c3))) {
      preventDefault(c3), v(a, Ec, void 0);
      return;
    }
    if ("F2" == eventKey(c3)) {
      preventDefault(c3), v(a, cd, void 0);
      return;
    }
    if ("F12" == eventKey(c3)) {
      preventDefault(c3), eventShiftKey(c3) ? v(a, Pc, void 0) : v(a, Oc, void 0);
      return;
    }
    if ("F8" == eventKey(c3)) {
      preventDefault(c3), v(a, Wc, void 0);
      return;
    }
    eventKey(c3) == zd && (b.suggestOpen = false, d2 = xc, setDisplay(b.suggestWidget, xc), b.hoverOpen = false, setDisplay(b.hoverWidget, xc), b.contextOpen = false, setDisplay(b.contextWidget, xc), b.findOpen = false, setDisplay(b.findWidget, xc), focusElement(b.textarea));
    dc(b, c3), w(a.cursorEmitter);
  }, c2), b.textarea.addEventListener("compositionend", function(a2) {
    a2 = a2.data + "", a2.length > 0 && S(b, a2);
  }, c2), b.textarea.addEventListener(Id, function(g) {
    g = g.target, g = g.value + "", g.length > 0 && S(b, g);
  }, c2), b.textarea.addEventListener("paste", function(a2) {
    ec(b, a2);
  }, c2), b.textarea.addEventListener("copy", function(a2) {
    bb(b, a2);
  }, c2), b.textarea.addEventListener("cut", function(a2) {
    fc(b, a2);
  }, c2), b.scrollable.addEventListener("scroll", function(a2) {
    b.scrollTop = +b.scrollable.scrollTop | 0, j(b);
  }, c2);
  let e = "mousedown";
  b.scrollable.addEventListener(e, function(c3) {
    b.contextOpen = false;
    var d2 = xc;
    setDisplay(b.contextWidget, xc), b.hoverOpen = false, setDisplay(b.hoverWidget, xc);
    var e2 = ca(b, eventClientX(c3), eventClientY(c3));
    d2 = e2.lineNumber, e2 = e2.column;
    var f = eventDetail(c3);
    b.mouseSelecting = true, b.mouseAnchorLine = d2, b.mouseAnchorColumn = e2, f >= 3 ? n(b, d2, 1, d2, o(b.model, d2) + 1 | 0) : 2 == f ? (v(a, qd, void 0), m(b, d2, e2), v(a, kd, void 0)) : eventShiftKey(c3) ? n(b, b.selection.selectionStartLineNumber, b.selection.selectionStartColumn, d2, e2) : m(b, d2, e2), focusElement(b.textarea), j(b), w(a.cursorEmitter);
  }, c2), b.margin.addEventListener(e, function(a2) {
    ba(b, ca(b, eventClientX(a2), eventClientY(a2)).lineNumber);
  }, c2), b.root.addEventListener("contextmenu", function(a2) {
    preventDefault(a2), gb(b, eventClientX(a2), eventClientY(a2));
  }, c2), b.contextWidget.addEventListener(e, function(c3) {
    c3 = hostCall(c3.target, "getAttribute", "data-cmd", void 0, void 0) + "", c3.length > 0 && c3 != Rd && v(a, c3, void 0), b.contextOpen = false, setDisplay(b.contextWidget, xc);
  }, c2), b.findInput.addEventListener(d, function(a2) {
    eventKey(a2) == Yc && (preventDefault(a2), fa(b, eventCtrlKey(a2))), eventKey(a2) == zd && (preventDefault(a2), b.findOpen = false, setDisplay(b.findWidget, xc), focusElement(b.textarea));
  }, c2), b.gotoInput.addEventListener(d, function(a2) {
    eventKey(a2) == Yc && (preventDefault(a2), hc(b));
  }, c2), b.renameInput.addEventListener(d, function(a2) {
    eventKey(a2) == Yc && (preventDefault(a2), ic(b));
  }, c2), b.scrollable.addEventListener("mousemove", function(c3) {
    if (b.mouseSelecting) {
      c3 = ca(b, eventClientX(c3), eventClientY(c3)), n(b, b.mouseAnchorLine, b.mouseAnchorColumn, c3.lineNumber, c3.column), j(b), w(a.cursorEmitter);
      return;
    }
    if (eventCtrlKey(c3)) {
      var D2 = ca(b, eventClientX(c3), eventClientY(c3));
      m(b, D2.lineNumber, D2.column), ya(b);
    }
  }, c2), b.scrollable.addEventListener("mouseup", function(a2) {
    b.mouseSelecting = false;
  }, c2), b.root.addEventListener("mouseleave", function(a2) {
    b.mouseSelecting = false;
  }, c2);
}
function pc(a) {
  var b = +a.view.root.clientWidth | 0, c2 = +a.view.root.clientHeight | 0;
  b < 1 && (b = a.view.width), c2 < 1 && (c2 = a.view.height), a = a.view, a.width = b, a.height = c2, setStyle(a.root, od, b.toString(10) + Xc), setStyle(a.root, Sc, c2.toString(10) + Xc), j(a);
}
var v = /* @__PURE__ */ (function() {
  function a(a2, b2) {
    var d2 = a2.buffer, e2 = A(d2, d2.root);
    d2 = hostGetOffsetAt(a2.buffer, b2.lineNumber, b2.column, i);
    if (d2 >= e2.length) return null;
    var c3 = e2.charAt(d2), f2 = "([{", g2 = ")]}", h2 = f2.indexOf(c3);
    if (h2 >= 0) {
      c3 = d2 + 1 | 0, d2 = 1;
      while (c3 < e2.length) {
        var j2 = e2.charAt(c3);
        if (j2 == f2.charAt(h2)) {
          d2 = d2 + 1 | 0;
        } else {
          if (j2 == g2.charAt(h2)) {
            if (0 == d2 - 1 | 0) {
              a2 = hostGetPositionAt(a2.buffer, c3, i), a2 = a2.column + 1 | 0, e2 = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 }, u(e2, b2.lineNumber, b2.column, a2.lineNumber, a2);
              return e2;
            }
          }
        }
        c3 = c3 + 1 | 0;
      }
      return null;
    }
    h2 = g2.indexOf(c3);
    if (h2 >= 0) {
      c3 = d2 - 1 | 0, d2 = 1;
      while (c3 >= 0) {
        j2 = e2.charAt(c3);
        if (j2 == g2.charAt(h2)) {
          d2 = d2 + 1 | 0;
        } else {
          if (j2 == f2.charAt(h2)) {
            if (0 == d2 - 1 | 0) {
              a2 = hostGetPositionAt(a2.buffer, c3, i), b2 = b2.column + 1 | 0, e2 = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 }, u(e2, a2.lineNumber, a2.column, b2.lineNumber, b2);
              return e2;
            }
          }
        }
        c3--;
      }
    }
    return null;
  }
  function b(a2, b2, c3) {
    c3 = Q(c3);
    var d2 = "/*", h2 = "*/";
    c3 && c3.blockCommentStart.length > 0 && (d2 = c3.blockCommentStart, h2 = c3.blockCommentEnd), c3 = La(a2.buffer, b2), 0 == c3.indexOf(d2) && c3.length >= (d2.length + h2.length | 0) ? l(a2, [p(b2.startLineNumber, b2.startColumn, b2.endLineNumber, b2.endColumn, r(c3, d2.length, c3.length - h2.length))], true) : l(a2, [p(b2.startLineNumber, b2.startColumn, b2.endLineNumber, b2.endColumn, d2 + c3 + h2)], true);
  }
  function c2(a2) {
    var c3, b2, e2, f2, h2 = q(a2), d2 = 1;
    while (d2 <= h2) {
      c3 = k(a2, d2), b2 = c3.indexOf("http://"), b2 < 0 && (b2 = c3.indexOf("https://"));
      if (b2 >= 0) {
        f2 = 0;
        while ((b2 + f2 | 0) < c3.length) {
          e2 = c3.charAt(b2 + f2 | 0);
          if (e2 == dd || e2 == Dd || ")" == e2 || '"' == e2) break;
          f2 = f2 + 1 | 0;
        }
        u({ startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 }, d2, b2 + 1 | 0, d2, (b2 + f2 | 0) + 1 | 0);
      }
      d2 = d2 + 1 | 0;
    }
  }
  function d(a2, b2) {
    var e2, f2, g2 = [], c3 = B(k(a2, b2)), d2 = 1;
    while (d2 < b2) e2 = k(a2, d2), f2 = B(e2), f2 < c3 && f2 != e2.length && (g2.push(e2), c3 = f2), d2 = d2 + 1 | 0;
    return g2;
  }
  function e(e2, a2) {
    if (a2) return f(e2);
    a2 = emptyBuf();
    var b2, c3 = 0;
    while (c3 < e2.length) b2 = e2.charCodeAt(c3), a2 = b2 >= 65 && b2 <= 90 ? a2 + String.fromCharCode(b2 + 32) : a2 + e2.charAt(c3), c3++;
    return a2;
  }
  function f(e2) {
    var b2, a2 = emptyBuf(), c3 = 0;
    while (c3 < e2.length) b2 = e2.charCodeAt(c3), a2 = b2 >= 97 && b2 <= 122 ? a2 + String.fromCharCode(b2 - 32) : a2 + e2.charAt(c3), c3++;
    return a2;
  }
  function g(b2) {
    b2.replaceQuery = inputGetValue(b2.replaceInput);
    var a2 = xa(b2);
    if (0 == a2.length) return;
    b2.findIndex >= a2.length && (b2.findIndex = 0), a2 = a2[b2.findIndex], l(b2.model, [p(a2[0].startLineNumber, a2[0].startColumn, a2[0].endLineNumber, a2[0].endColumn, b2.replaceQuery)], true), j(b2), fa(b2, false);
  }
  function h(b2) {
    b2.replaceQuery = inputGetValue(b2.replaceInput);
    var d2 = xa(b2), c3 = d2.length - 1;
    while (c3 >= 0) {
      var a2 = d2[c3], e2 = b2.model;
      l(e2, [p(a2[0].startLineNumber, a2[0].startColumn, a2[0].endLineNumber, a2[0].endColumn, b2.replaceQuery)], true), c3--;
    }
    j(b2);
  }
  function o2(b2) {
    let a2 = s(k(b2.model, b2.selection.positionLineNumber), b2.selection.positionColumn);
    b2.renameWord = a2[0], b2.renameOpen = true, inputSetValue(b2.renameInput, a2[0]), setDisplay(b2.renameWidget, yc), focusElement(b2.renameInput);
  }
  function t2(b2) {
    var a2 = J(b2.model, s(k(b2.model, b2.selection.positionLineNumber), b2.selection.positionColumn)[0], true), d2 = emptyBuf(), c3 = 0;
    while (c3 < a2.length && c3 < 20) d2 = d2 + '<div data-line="' + a2[c3][0].startLineNumber.toString(10) + Ed + a2[c3][0].startLineNumber.toString(10) + ": " + (a2[c3][1][0] || "") + Rc, c3++;
    setInnerHTML(b2.hoverWidget, d2), setDisplay(b2.hoverWidget, yc), b2.hoverOpen = true;
  }
  function x2(b2) {
    var c3 = s(k(b2.model, b2.selection.positionLineNumber), b2.selection.positionColumn), a2 = L(b2);
    0 == a2.length && (a2 = c3[0], n(b2, b2.selection.positionLineNumber, c3[1], b2.selection.positionLineNumber, c3[2]));
    var d2 = J(b2.model, a2, true);
    c3 = 0;
    while (c3 < d2.length) {
      a2 = d2[c3];
      var e2 = a2[0].startLineNumber == b2.selection.startLineNumber && a2[0].startColumn == b2.selection.startColumn;
      if (!e2) {
        b2.extraCursors.push(b2.selection), n(b2, a2[0].startLineNumber, a2[0].startColumn, a2[0].endLineNumber, a2[0].endColumn), j(b2);
        return;
      }
      c3++;
    }
  }
  function y2(f2, i2, q2) {
    if ("actions.find" == i2 || "editor.action.startFindAction" == i2) {
      ea(f2);
      return true;
    }
    if ("editor.action.startFindReplaceAction" == i2) {
      ea(f2);
      return true;
    }
    if ("editor.action.nextMatchFindAction" == i2) {
      fa(f2, false);
      return true;
    }
    if ("editor.action.previousMatchFindAction" == i2) {
      fa(f2, true);
      return true;
    }
    if ("editor.action.replaceOne" == i2) {
      g(f2);
      return true;
    }
    if ("editor.action.replaceAll" == i2) {
      h(f2);
      return true;
    }
    if ("closeFindWidget" == i2) {
      f2.findOpen = false, setDisplay(f2.findWidget, xc), focusElement(f2.textarea);
      return true;
    }
    if (i2 == Qc) {
      db(f2);
      return true;
    }
    if ("acceptSelectedSuggestion" == i2) {
      eb(f2);
      return true;
    }
    if ("hideSuggestWidget" == i2) {
      f2.suggestOpen = false, setDisplay(f2.suggestWidget, xc);
      return true;
    }
    if ("editor.action.showHover" == i2) {
      ya(f2);
      return true;
    }
    if (i2 == $c) {
      f2.gotoOpen = true, setDisplay(f2.gotoWidget, yc), focusElement(f2.gotoInput);
      return true;
    }
    if (i2 == cd) {
      o2(f2);
      return true;
    }
    if ("editor.action.triggerParameterHints" == i2) {
      jc(f2);
      return true;
    }
    if ("closeParameterHints" == i2) {
      setDisplay(f2.paramWidget, xc);
      return true;
    }
    if (i2 == Vc) {
      Ta(f2.model, f2.selection.positionLineNumber, f2.languageId), j(f2);
      return true;
    }
    if ("editor.action.blockComment" == i2) {
      q2 = f2.selection.startLineNumber;
      var r2 = f2.selection.startColumn, v2 = f2.selection.endLineNumber, w2 = f2.selection.endColumn, y3 = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
      u(y3, q2, r2, v2, w2), b(f2.model, y3, f2.languageId), j(f2);
      return true;
    }
    if ("editor.action.jumpToBracket" == i2) {
      r2 = f2.selection.positionColumn, i2 = { lineNumber: 0, column: 0 }, i2.lineNumber = f2.selection.positionLineNumber, i2.column = r2, i2 = a(f2.model, i2), i2 && (m(f2, i2.endLineNumber, i2.endColumn), j(f2));
      return true;
    }
    if ("editor.action.selectToBracket" == i2) {
      r2 = f2.selection.positionColumn, i2 = { lineNumber: 0, column: 0 }, i2.lineNumber = f2.selection.positionLineNumber, i2.column = r2, i2 = a(f2.model, i2), i2 && (n(f2, i2.startLineNumber, i2.startColumn, i2.endLineNumber, i2.endColumn), j(f2));
      return true;
    }
    if ("editor.fold" == i2) {
      ba(f2, f2.selection.positionLineNumber);
      return true;
    }
    if ("editor.unfold" == i2) {
      ba(f2, f2.selection.positionLineNumber);
      return true;
    }
    if ("editor.foldAll" == i2) {
      Wa(f2, true);
      return true;
    }
    if ("editor.unfoldAll" == i2) {
      Wa(f2, false);
      return true;
    }
    if ("editor.action.formatDocument" == i2) {
      lc(f2);
      return true;
    }
    if (i2 == Oc || "editor.action.revealDefinition" == i2 || "editor.action.goToDeclaration" == i2) {
      kc(f2);
      return true;
    }
    if (i2 == Pc || "editor.action.referenceSearch.trigger" == i2 || "editor.action.peekDefinition" == i2) {
      t2(f2);
      return true;
    }
    if (i2 == Ec) {
      x2(f2);
      return true;
    }
    if ("editor.action.selectHighlights" == i2 || "editor.action.changeAll" == i2) {
      i2 = J(f2.model, s(k(f2.model, f2.selection.positionLineNumber), f2.selection.positionColumn)[0], true), i2.length > 0 && (n(f2, i2[0][0].startLineNumber, i2[0][0].startColumn, i2[i2.length - 1][0].endLineNumber, i2[i2.length - 1][0].endColumn), j(f2));
      return true;
    }
    if ("editor.action.wordHighlight.trigger" == i2) {
      r2 = f2.selection.positionColumn, i2 = { lineNumber: 0, column: 0 }, i2.lineNumber = f2.selection.positionLineNumber, i2.column = r2, Ua(f2.model, i2);
      return true;
    }
    if ("editor.action.openLink" == i2) {
      c2(f2.model);
      return true;
    }
    if ("editor.action.transformToUppercase" == i2) {
      i2 = L(f2), i2.length > 0 && (q2 = f2.model, r2 = f2.selection.startLineNumber, v2 = f2.selection.startColumn, w2 = f2.selection.endLineNumber, y3 = f2.selection.endColumn, l(q2, [p(r2, v2, w2, y3, e(i2, true))], true), j(f2));
      return true;
    }
    if ("editor.action.transformToLowercase" == i2) {
      i2 = L(f2), i2.length > 0 && (q2 = f2.model, r2 = f2.selection.startLineNumber, v2 = f2.selection.startColumn, w2 = f2.selection.endLineNumber, y3 = f2.selection.endColumn, l(q2, [p(r2, v2, w2, y3, e(i2, false))], true), j(f2));
      return true;
    }
    if ("editor.action.insertSnippet" == i2) {
      S(f2, Sa(q2));
      return true;
    }
    if ("editor.action.quickOutline" == i2 || "editor.action.gotoSymbol" == i2) {
      t2(f2);
      return true;
    }
    if (i2 == Wc || "editor.action.gotoErrorNext" == i2 || "editor.action.marker.nextInFiles" == i2) {
      z2(f2, false);
      return true;
    }
    if ("editor.action.marker.prev" == i2 || "editor.action.gotoErrorPrev" == i2) {
      z2(f2, true);
      return true;
    }
    if ("editor.action.quickFix" == i2 || "editor.action.codeAction" == i2) {
      mc(f2);
      return true;
    }
    if ("editor.action.smartSelect.expand" == i2) {
      r2 = f2.selection.positionColumn, i2 = { lineNumber: 0, column: 0 }, i2.lineNumber = f2.selection.positionLineNumber, i2.column = r2, i2 = a(f2.model, i2), i2 ? (n(f2, i2.startLineNumber, i2.startColumn, i2.endLineNumber, i2.endColumn), j(f2)) : (i2 = s(k(f2.model, f2.selection.positionLineNumber), f2.selection.positionColumn), n(f2, f2.selection.positionLineNumber, i2[1], f2.selection.positionLineNumber, i2[2]), j(f2));
      return true;
    }
    if ("editor.action.smartSelect.shrink" == i2) {
      m(f2, f2.selection.positionLineNumber, f2.selection.positionColumn), j(f2);
      return true;
    }
    if ("editor.toggleFold" == i2 || "editor.foldRecursively" == i2 || "editor.unfoldRecursively" == i2) {
      ba(f2, f2.selection.positionLineNumber);
      return true;
    }
    if ("editor.action.showContextMenu" == i2) {
      gb(f2, 80, f2.selection.positionLineNumber * f2.lineHeight | 0);
      return true;
    }
    if ("editor.action.inlayHints.toggle" == i2) {
      r2 = d(f2.model, f2.selection.positionLineNumber), q2 = 0;
      while (q2 < r2.length) q2++;
      setTextContent(f2.stickyWidget, emptyBuf() + (r2[q2] || "") + Ac), setDisplay(f2.stickyWidget, yc);
      return true;
    }
    return false;
  }
  function z2(b2, a2) {
    var f2 = qa(ma(b2.model.uri), 0);
    if (0 == f2.length) return;
    for (var e2, d2 = b2.selection.positionLineNumber, g2 = f2[0][3], c3 = 0; ; c3++) {
      if (c3 >= f2.length) {
        e2 = g2;
        break;
      }
      e2 = f2[c3][3];
      if (!a2 && e2 > d2) break;
      a2 && e2 < d2 && (g2 = e2);
    }
    !a2 && e2 <= d2 && (e2 = f2[0][3]);
    m(b2, e2, 1), ya(b2), j(b2);
  }
  return function(a2, b2, c3) {
    var d2, e2 = Ic;
    c3 && c3.text && (e2 = c3.text + ""), c3 && c3.lineNumber && (d2 = +c3.lineNumber | 0, c3 = c3.column ? +c3.column | 0 : 1, m(a2.view, d2, c3), j(a2.view));
    if (cc(a2.view, b2, e2)) {
      w(a2.cursorEmitter);
      return;
    }
    if (y2(a2.view, b2, e2)) {
      w(a2.cursorEmitter);
      return;
    }
    c3 = 0;
    while (c3 < a2.actions.length) {
      if (a2.actions[c3].id == b2) {
        a2.actions[c3].run(a2);
        return;
      }
      c3++;
    }
  };
})();
var qc = /* @__PURE__ */ (function() {
  function a(a2, b2, c2) {
    if (!a2) return c2;
    a2 = a2[b2];
    return !a2 ? c2 : a2 + "" == nd ? false : a2 + "" == Fd ? false : true;
  }
  function b(a2, b2, c2) {
    if (!a2) return c2;
    a2 = a2[b2];
    return !a2 ? c2 : +a2 | 0;
  }
  return function(c2, d) {
    var e = M(d, "theme", c2.view.theme), f = c2.view;
    f.showLineNumbers = a(d, "lineNumbers", f.showLineNumbers), f.readOnly = a(d, "readOnly", f.readOnly), f.tabSize = b(d, "tabSize", f.tabSize), f.insertSpaces = a(d, "insertSpaces", f.insertSpaces), f.fontSize = b(d, "fontSize", f.fontSize), f.wordWrap = a(d, "wordWrap", f.wordWrap), d && d.minimap && (d = d.minimap, f = c2.view, f.showMinimap = a(d, "enabled", f.showMinimap)), e != c2.view.theme ? (c2 = c2.view, c2.theme = e, R(c2), j(c2)) : j(c2.view);
  };
})();
function rc(a, b, c2, d) {
  a[0] = b, a[1] = c2, a[2] = d, a[3] = [];
}
function za(b, g, a, c2) {
  pa();
  var d = ga;
  ga = ga + 1 | 0;
  var e = Ic, f = "/model/" + d.toString(10);
  d = { scheme: "", authority: "", path: "", query: "", fragment: "" }, d.scheme = Vd, d.authority = Ic, d.path = f, d.query = Ic, d.fragment = Ic, e = { buffer: null, uri: null, languageId: "", stack: null, decorations: null, onDidChangeContent: null, versionId: 0, decoScratchIdx: 0 }, Cb(e, g, a, d), ib.push(e), b = Tb(b, e), b.theme = c2, b.languageId = a, R(b), j(b), g = { view: null, contentEmitter: null, cursorEmitter: null, disposed: false, actions: [], modelFacade: null }, nc(g, b), ha.push(g), w(kb);
  return g;
}
function sc(a, b) {
  var g = M(b, Zc, Ic);
  g = za(a, g, M(b, Wd, Mc), M(b, "theme", V)), qc(g, b);
  if (b && b.model) {
  }
  j(g.view);
  return g;
}
function tc(a, b) {
  let d = "div", c2 = document.createElement(d);
  d = document.createElement(d);
  let e = wd;
  setStyle(a, Cc, wd);
  let f = "1 1 50%";
  setStyle(c2, wd, f), setStyle(d, wd, f), a.appendChild(c2), a.appendChild(d), f = M(b, "original", Ic), b = M(b, Wd, Mc), c2 = za(c2, f, b, V), d = za(d, M(b, "modified", e), b, V), b = [null, null, null, []], rc(b, a, c2, d), jb.push(b), w(lb);
  return b;
}
function uc(a) {
  var b, c2 = 0;
  while (c2 < ha.length) b = ha[c2].view, b.theme = a, R(b), j(b), c2++;
}
function vc(a) {
  a = a.view.model.buffer;
  return A(a, a.root);
}
function hb(a, b, c2) {
  v(a, b, c2);
}
function wc(a, b, c2) {
  pa();
  var d = td;
  a = sc(a, { __proto__: null, value: 'function hello(name) {\n  const msg = "hi " + name;\n  return msg;\n}\n', language: bd, theme: td, lineNumbers: "on" }), m(a.view, 1, 23), j(a.view), hb(a, xd, { __proto__: null, text: "!" }), hb(a, "undo", { __proto__: null }), uc(td), pc(a);
  var e = J(a.view.model, "msg", true), f = Ra(a.view.model);
  m(a.view, 2, 10), j(a.view);
  var g = a.view.model, h = a.view.selection.positionColumn;
  d = { lineNumber: 0, column: 0 }, d.lineNumber = a.view.selection.positionLineNumber, d.column = h, g = Ua(g, d), d = a.view.selection.positionColumn, h = s(k(a.view.model, a.view.selection.positionLineNumber), d)[0], d = q(a.view.model), 2 > d || (d = 2), m(a.view, d, 1), ta(a.view, d), j(a.view), Ta(a.view.model, a.view.selection.positionLineNumber, a.view.languageId), j(a.view), b = tc(b, { __proto__: null, original: "a\nb\nc\n", modified: "a\nx\nc\n", language: Mc }), d = b[1].view.model.buffer, d = A(d, d.root), b = b[2].view.model.buffer, b = hostComputeLineDiff(d, A(b, b.root)), setTextContent(c2, "value=" + vc(a).length.toString(10) + " matches=" + e.length.toString(10) + " folds=" + f.length.toString(10) + " highlights=" + g.length.toString(10) + " hover=" + h + " diffs=" + b.length.toString(10));
}
function F(a, b, f, c2) {
  let d = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
  u(d, a, b, f, c2), b = concat2(emptyBuf(), Ic), a = { range: null, text: "", identifier: 0 }, a.range = d, a.text = b, a.identifier = 0;
  return a;
}
var ae = { bufferIndex: 0, start: { line: 0, column: 0 }, end: { line: 0, column: 0 }, lineFeedCnt: 0, length: 0 };
var be = { parent: null, left: null, right: null, color: 0, piece: null, size_left: 0, lf_left: 0, alive: false };
ia(be, ae, 0);
var i = be;
var ga = 1;
var ce = { startLineNumber: 0, startColumn: 0, endLineNumber: 0, endColumn: 0 };
u(ce, 1, 1, 1, 1);
ae = { id: 0, range: null, className: "", hoverMessage: "", isWholeLine: false };
ae.id = 0;
ae.range = ce;
ae.className = ae.hoverMessage = Ic;
ae.isWholeLine = false;
be = { parent: null, left: null, right: null, color: 0, deco: null, maxEndLine: 0, maxEndColumn: 0, alive: false };
yb(be, ae);
var T = be;
var ib = [];
var G = [];
var Aa = false;
var Ba = [];
var U = [];
var ha = [];
var jb = [];
ae = { listeners: [], disposed: false };
ae.listeners = [];
ae.disposed = false;
var kb = ae;
ae = { listeners: [], disposed: false };
ae.listeners = [];
ae.disposed = false;
var lb = ae;
export {
  sc as create,
  vc as editorGetValue,
  wc as runDemo,
  uc as setTheme
};
