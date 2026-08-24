/* 8.  The static dictionary.
   122,784 bytes holding 13,504 words of length 4..24, addressed by
   (length, index), and 121 transforms that wrap, cut or upper-case a word.
   A copy whose distance is past the end of the window is not a copy at all:
   it is dictionary entry number (distance - max_distance - 1). */
(function (BM) {
  "use strict";
  const T = BM.tables;

  function base64ToBytes(b64) {
    if (typeof atob === "function") {
      const bin = atob(b64);
      const out = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
      return out;
    }
    return new Uint8Array(Buffer.from(b64, "base64"));
  }

  const latin1 = (bytes) => {
    let s = "";
    for (let i = 0; i < bytes.length; i += 4096) {
      s += String.fromCharCode.apply(null, bytes.subarray(i, i + 4096));
    }
    return s;
  };

  class Dictionary {
    constructor(data) {
      this.bytes = base64ToBytes(data.dictionaryBase64);
      this.sizeBits = data.sizeBitsByLength;
      this.offsets = data.offsetsByLength;
      this.transforms = data.transforms;
      this.prefixSuffix = data.prefixSuffix;
      this.typeNames = data.transformTypeNames;
      this.text = latin1(this.bytes);
      this._exact = null;
      this._sorted = null;
    }
    countFor(len) { return this.sizeBits[len] ? 1 << this.sizeBits[len] : 0; }
    /* Raw word bytes at (length, index). */
    word(len, index) {
      const start = this.offsets[len] + index * len;
      return this.bytes.subarray(start, start + len);
    }
    wordText(len, index) {
      const start = this.offsets[len] + index * len;
      return this.text.slice(start, start + len);
    }

    /* --- transforms ---------------------------------------------------- */
    transformParts(t) {
      const [prefixId, type, suffixId] = this.transforms[t];
      return { prefix: this.prefixSuffix[prefixId], type, typeName: this.typeNames[type],
               suffix: this.prefixSuffix[suffixId] };
    }
    describeTransform(t) {
      const { prefix, typeName, suffix } = this.transformParts(t);
      const q = (s) => (s === "" ? "" : JSON.stringify(s));
      const bits = [];
      if (prefix) bits.push(q(prefix) + " +");
      bits.push(typeName === "IDENTITY" ? "word" : typeName + "(word)");
      if (suffix) bits.push("+ " + q(suffix));
      return bits.join(" ");
    }
    /* 8.  Exactly BrotliTransformDictionaryWord, on strings. */
    applyTransform(wordText, t) {
      const { prefix, type, suffix } = this.transformParts(t);
      let body = wordText;
      if (type <= 9) {
        body = body.slice(0, Math.max(0, body.length - type)); /* OMIT_LAST_n */
      } else if (type >= 12 && type <= 20) {
        body = body.slice(Math.min(body.length, type - 11)); /* OMIT_FIRST_n */
      }
      if (type === 10) body = upperCaseFirst(body);
      else if (type === 11) body = upperCaseAll(body);
      return prefix + body + suffix;
    }
    /* Dictionary entry id -> the bytes it expands to. */
    expand(len, wordIndex, transformIndex) {
      return this.applyTransform(this.wordText(len, wordIndex), transformIndex);
    }
    /* (length, index, transform) <-> the id encoded in the distance. */
    entryId(len, wordIndex, transformIndex) {
      return transformIndex * this.countFor(len) + wordIndex;
    }
    decodeId(len, id) {
      const shift = this.sizeBits[len];
      return { wordIndex: id & ((1 << shift) - 1), transformIndex: id >>> shift };
    }

    /* --- indexes for the encoder and the explorer ---------------------- */
    exactIndex() {
      if (this._exact) return this._exact;
      const byLength = new Map();
      for (let len = 4; len <= 24; len++) {
        const map = new Map();
        const n = this.countFor(len);
        for (let i = 0; i < n; i++) {
          const w = this.wordText(len, i);
          if (!map.has(w)) map.set(w, i);
        }
        byLength.set(len, map);
      }
      this._exact = byLength;
      return byLength;
    }
    /* Word indexes per length, sorted by text, so a known prefix maps to a
       contiguous range (that is how OMIT_LAST_n candidates are found). */
    sortedIndex() {
      if (this._sorted) return this._sorted;
      const byLength = new Map();
      for (let len = 4; len <= 24; len++) {
        const n = this.countFor(len);
        const idx = new Int32Array(n);
        for (let i = 0; i < n; i++) idx[i] = i;
        const arr = Array.from(idx).sort((a, b) => {
          const x = this.wordText(len, a), y = this.wordText(len, b);
          return x < y ? -1 : x > y ? 1 : a - b;
        });
        byLength.set(len, arr);
      }
      this._sorted = byLength;
      return byLength;
    }
    prefixRange(len, prefix) {
      const arr = this.sortedIndex().get(len);
      if (!arr) return [];
      const cmp = (i) => {
        const w = this.wordText(len, i).slice(0, prefix.length);
        return w < prefix ? -1 : w > prefix ? 1 : 0;
      };
      let lo = 0, hi = arr.length;
      while (lo < hi) { const mid = (lo + hi) >> 1; if (cmp(arr[mid]) < 0) lo = mid + 1; else hi = mid; }
      const start = lo;
      hi = arr.length;
      while (lo < hi) { const mid = (lo + hi) >> 1; if (cmp(arr[mid]) <= 0) lo = mid + 1; else hi = mid; }
      return arr.slice(start, lo);
    }

    /* Free-text search for the explorer. */
    search(query, limit = 60) {
      const out = [];
      if (!query) return out;
      for (let len = 4; len <= 24 && out.length < limit; len++) {
        const n = this.countFor(len);
        for (let i = 0; i < n && out.length < limit; i++) {
          const w = this.wordText(len, i);
          if (w.includes(query)) out.push({ len, index: i, word: w, exact: w === query });
        }
      }
      out.sort((a, b) => (b.exact - a.exact) || a.len - b.len);
      return out;
    }

    /* Which transformed dictionary entries produce the text at `pos`?
       Covered here: identity, affix-only, OMIT_LAST_n and UPPERCASE_FIRST /
       UPPERCASE_ALL with affixes — the families a real encoder searches. */
    matchesAt(text, pos, opts = {}) {
      const maxLen = opts.maxLen ?? 24;
      const found = [];
      const seen = new Set();
      const push = (len, wordIndex, t, produced) => {
        const key = len + ":" + wordIndex + ":" + t;
        if (seen.has(key)) return;
        seen.add(key);
        found.push({ len, wordIndex, transform: t, produced, matched: produced.length,
                     id: this.entryId(len, wordIndex, t) });
      };
      for (let t = 0; t < this.transforms.length; t++) {
        const { prefix, type, suffix } = this.transformParts(t);
        if (prefix && text.slice(pos, pos + prefix.length) !== prefix) continue;
        const bodyStart = pos + prefix.length;
        for (let len = 4; len <= maxLen; len++) {
          if (!this.countFor(len)) continue;
          let bodyLen = len;
          if (type <= 9) bodyLen = len - type;
          else if (type >= 12 && type <= 20) bodyLen = len - (type - 11);
          else if (type > 11 && type < 12) continue;
          if (bodyLen <= 0) continue;
          if (type >= 12) continue; /* OMIT_FIRST_n needs a suffix index; skipped */
          const body = text.slice(bodyStart, bodyStart + bodyLen);
          if (body.length < bodyLen) continue;
          if (suffix && text.slice(bodyStart + bodyLen, bodyStart + bodyLen + suffix.length) !== suffix) continue;
          let candidates;
          if (type === 0) {
            const hit = this.exactIndex().get(len).get(body);
            candidates = hit === undefined ? [] : [hit];
          } else if (type <= 9) {
            candidates = this.prefixRange(len, body);
          } else {
            /* upper-casing is not invertible byte-for-byte, so try the
               lower-cased spelling and verify by re-applying. */
            candidates = [];
            const guess = lowerCaseFirstOrAll(body, type);
            const hit = this.exactIndex().get(len).get(guess);
            if (hit !== undefined) candidates = [hit];
          }
          for (const wordIndex of candidates) {
            const produced = this.applyTransform(this.wordText(len, wordIndex), t);
            if (text.slice(pos, pos + produced.length) === produced && produced.length >= 4) {
              push(len, wordIndex, t, produced);
            }
          }
        }
      }
      found.sort((a, b) => b.matched - a.matched);
      return found;
    }
  }

  function upperCaseFirst(s) {
    if (!s) return s;
    const c = s.charCodeAt(0);
    if (c < 0xc0) return (c >= 97 && c <= 122 ? String.fromCharCode(c ^ 32) : s[0]) + s.slice(1);
    if (c < 0xe0) return s[0] + String.fromCharCode(s.charCodeAt(1) ^ 32) + s.slice(2);
    return s.slice(0, 2) + String.fromCharCode(s.charCodeAt(2) ^ 5) + s.slice(3);
  }
  function upperCaseAll(s) {
    let out = "", i = 0;
    while (i < s.length) {
      const c = s.charCodeAt(i);
      if (c < 0xc0) { out += c >= 97 && c <= 122 ? String.fromCharCode(c ^ 32) : s[i]; i += 1; }
      else if (c < 0xe0) { out += s[i] + String.fromCharCode(s.charCodeAt(i + 1) ^ 32); i += 2; }
      else { out += s.slice(i, i + 2) + String.fromCharCode(s.charCodeAt(i + 2) ^ 5); i += 3; }
    }
    return out;
  }
  function lowerCaseFirstOrAll(s, type) {
    const lower = (ch) => {
      const c = ch.charCodeAt(0);
      return c >= 65 && c <= 90 ? String.fromCharCode(c | 32) : ch;
    };
    if (type === 10) return lower(s[0]) + s.slice(1);
    return Array.from(s).map(lower).join("");
  }

  BM.Dictionary = Dictionary;
  BM.dictionary = function () {
    if (!BM._dict) BM._dict = new Dictionary(BM.data);
    return BM._dict;
  };
  BM.latin1 = latin1;
  BM.base64ToBytes = base64ToBytes;
  BM.bytesFromLatin1 = (s) => {
    const out = new Uint8Array(s.length);
    for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i) & 0xff;
    return out;
  };
})(globalThis.BM || (globalThis.BM = {}));
