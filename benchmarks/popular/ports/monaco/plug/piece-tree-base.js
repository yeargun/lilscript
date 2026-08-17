import { Position } from "../../../node_modules/monaco-editor/esm/vs/editor/common/core/position.js";
import { Range } from "../../../node_modules/monaco-editor/esm/vs/editor/common/core/range.js";
import { FindMatch } from "../../../node_modules/monaco-editor/esm/vs/editor/common/model.js";
import * as lil from "../../../build/monaco-layers/piece-tree.raw.js";

function createUintArray(arr) {
  const r = arr[arr.length - 1] < 65536 ? new Uint16Array(arr.length) : new Uint32Array(arr.length);
  r.set(arr, 0);
  return r;
}

export function createLineStartsFast(str, readonly = true) {
  const r = [0];
  for (let i = 0, len = str.length; i < len; i++) {
    const chr = str.charCodeAt(i);
    if (chr === 13) {
      if (i + 1 < len && str.charCodeAt(i + 1) === 10) {
        r.push(i + 2);
        i++;
      } else {
        r.push(i + 1);
      }
    } else if (chr === 10) {
      r.push(i + 1);
    }
  }
  return readonly ? createUintArray(r) : r;
}

export function createLineStarts(r, str) {
  r.length = 0;
  r[0] = 0;
  let cr = 0;
  let lf = 0;
  let crlf = 0;
  let isBasicASCII = true;
  for (let i = 0, len = str.length; i < len; i++) {
    const chr = str.charCodeAt(i);
    if (chr === 13) {
      if (i + 1 < len && str.charCodeAt(i + 1) === 10) {
        crlf++;
        r.push(i + 2);
        i++;
      } else {
        cr++;
        r.push(i + 1);
      }
    } else if (chr === 10) {
      lf++;
      r.push(i + 1);
    } else if (isBasicASCII && chr !== 9 && (chr < 32 || chr > 126)) {
      isBasicASCII = false;
    }
  }
  const lineStarts = createUintArray(r);
  r.length = 0;
  return { lineStarts, cr, lf, crlf, isBasicASCII };
}

export class StringBuffer {
  constructor(buffer, lineStarts) {
    this.buffer = buffer;
    this.lineStarts = lineStarts;
  }
}

export class Piece {
  constructor(bufferIndex, start, end, lineFeedCnt, length) {
    this.bufferIndex = bufferIndex;
    this.start = start;
    this.end = end;
    this.lineFeedCnt = lineFeedCnt;
    this.length = length;
  }
}

function chunksToValue(chunks, eol) {
  let value = "";
  for (const chunk of chunks ?? []) {
    value += chunk?.buffer ?? "";
  }
  return value.replace(/\r\n|\r|\n/g, eol);
}

export class PieceTreeBase {
  constructor(chunks, eol, _eolNormalized) {
    this._eol = eol || "\n";
    this._tree = lil.create(chunksToValue(chunks, this._eol), this._eol);
    globalThis.__lilPlugs = { ...(globalThis.__lilPlugs ?? {}), pieceTree: true };
  }

  create(chunks, eol, _eolNormalized) {
    this._eol = eol || "\n";
    this._tree = lil.create(chunksToValue(chunks, this._eol), this._eol);
  }

  getEOL() {
    return this._eol;
  }

  setEOL(newEOL) {
    const value = lil.getValue(this._tree).replace(/\r\n|\r|\n/g, newEOL);
    this._eol = newEOL;
    this._tree = lil.create(value, newEOL);
  }

  createSnapshot(BOM) {
    const value = String(BOM ?? "") + lil.getValue(this._tree);
    let sent = false;
    return {
      read() {
        if (sent) return null;
        sent = true;
        return value;
      },
    };
  }

  getOffsetAt(lineNumber, column) {
    return lil.getOffsetAt(this._tree, lineNumber, column);
  }

  getPositionAt(offset) {
    const pos = lil.getPositionAt(this._tree, offset | 0);
    return new Position(pos.lineNumber, pos.column);
  }

  getValueInRange(range, eol) {
    if (range.startLineNumber === range.endLineNumber && range.startColumn === range.endColumn) {
      return "";
    }
    const value = lil.getValueInRange(
      this._tree,
      range.startLineNumber,
      range.startColumn,
      range.endLineNumber,
      range.endColumn,
    );
    if (eol && eol !== this._eol) {
      return value.replace(/\r\n|\r|\n/g, eol);
    }
    return value;
  }

  getLinesContent() {
    const n = lil.getLineCount(this._tree);
    const out = [];
    for (let i = 1; i <= n; i++) out.push(lil.getLineContent(this._tree, i));
    return out;
  }

  getLength() {
    return lil.getLength(this._tree);
  }

  getLineCount() {
    return lil.getLineCount(this._tree);
  }

  getLineContent(lineNumber) {
    return lil.getLineContent(this._tree, lineNumber);
  }

  getLineCharCode(lineNumber, index) {
    return this.getLineContent(lineNumber).charCodeAt(index) || 0;
  }

  getLineLength(lineNumber) {
    return lil.getLineLength(this._tree, lineNumber);
  }

  getNearestChunk(offset) {
    const pos = lil.getPositionAt(this._tree, offset | 0);
    return this.getLineContent(pos.lineNumber).slice(pos.column - 1);
  }

  insert(offset, value) {
    lil.insert(this._tree, offset | 0, value ?? "");
  }

  delete(offset, cnt) {
    if ((cnt | 0) <= 0) return;
    lil.deleteRange(this._tree, offset | 0, cnt | 0);
  }

  findMatchesLineByLine(searchRange, searchData, captureMatches, limitResultCount) {
    const result = [];
    const limit = limitResultCount || 999;
    const startLine = searchRange.startLineNumber;
    const endLine = searchRange.endLineNumber;
    const simple = !captureMatches && searchData?.simpleSearch;
    const regex = searchData?.regex;
    for (let line = startLine; line <= endLine && result.length < limit; line++) {
      const content = this.getLineContent(line);
      const from = line === startLine ? Math.max(0, searchRange.startColumn - 1) : 0;
      const to = line === endLine ? Math.max(from, searchRange.endColumn - 1) : content.length;
      const slice = content.substring(from, to);
      if (simple) {
        const q = searchData.simpleSearch;
        let idx = 0;
        while (result.length < limit) {
          const at = slice.indexOf(q, idx);
          if (at < 0) break;
          const col = from + at + 1;
          result.push(new FindMatch(new Range(line, col, line, col + q.length), null));
          idx = at + Math.max(1, q.length);
        }
        continue;
      }
      if (!regex) continue;
      const flags = regex.flags.includes("g") ? regex.flags : regex.flags + "g";
      const re = new RegExp(regex.source, flags);
      let match = re.exec(slice);
      while (match && result.length < limit) {
        const col = from + match.index + 1;
        result.push(new FindMatch(new Range(line, col, line, col + match[0].length), captureMatches ? match : null));
        if (match[0].length === 0) re.lastIndex += 1;
        match = re.exec(slice);
      }
    }
    return result;
  }
}
