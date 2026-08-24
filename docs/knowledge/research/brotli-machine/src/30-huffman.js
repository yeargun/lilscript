/* 3.  Prefix codes.
   Brotli uses canonical prefix codes, exactly as DEFLATE does: symbols are
   sorted by (code length, symbol value), codes are handed out in increasing
   order within each length, and each code is written to the stream reversed so
   that reading LSB-first yields it MSB-first. */
(function (BM) {
  "use strict";
  const T = BM.tables;

  /* A decode table is just the per-length symbol counts plus the sorted
     symbols; decoding walks one bit at a time (puff's algorithm). */
  function buildDecodeTable(lengths) {
    const counts = new Int32Array(T.MAX_CODE_LENGTH + 1);
    for (const len of lengths) if (len) counts[len]++;
    const offsets = new Int32Array(T.MAX_CODE_LENGTH + 2);
    for (let len = 1; len <= T.MAX_CODE_LENGTH; len++) offsets[len + 1] = offsets[len] + counts[len];
    const total = offsets[T.MAX_CODE_LENGTH + 1];
    const symbols = new Int32Array(total);
    const cursor = offsets.slice();
    for (let sym = 0; sym < lengths.length; sym++) {
      const len = lengths[sym];
      if (len) symbols[cursor[len]++] = sym;
    }
    let maxLen = 0;
    for (let len = 1; len <= T.MAX_CODE_LENGTH; len++) if (counts[len]) maxLen = len;
    return { counts, symbols, lengths, maxLen, single: total === 1 ? symbols[0] : -1 };
  }

  function readSymbol(reader, table) {
    if (table.single >= 0) return table.single;
    let code = 0, first = 0, index = 0;
    for (let len = 1; len <= T.MAX_CODE_LENGTH; len++) {
      code |= reader.readBit();
      const count = table.counts[len];
      if (code - first < count) return table.symbols[index + (code - first)];
      index += count;
      first = (first + count) << 1;
      code <<= 1;
    }
    throw new BM.StreamError("no prefix code matched");
  }

  /* Encoder side: lengths -> reversed codes ready for the bit writer. */
  function buildEncodeTable(lengths) {
    const counts = new Int32Array(T.MAX_CODE_LENGTH + 1);
    for (const len of lengths) if (len) counts[len]++;
    const next = new Int32Array(T.MAX_CODE_LENGTH + 2);
    let code = 0;
    for (let len = 1; len <= T.MAX_CODE_LENGTH; len++) {
      code = (code + counts[len - 1]) << 1;
      next[len] = code;
    }
    const out = new Array(lengths.length).fill(null);
    for (let sym = 0; sym < lengths.length; sym++) {
      const len = lengths[sym];
      if (!len) continue;
      const value = next[len]++;
      out[sym] = { length: len, code: value, reversed: reverseBits(value, len) };
    }
    return out;
  }

  function reverseBits(value, count) {
    let out = 0;
    for (let i = 0; i < count; i++) out |= ((value >> i) & 1) << (count - 1 - i);
    return out >>> 0;
  }

  /* Length-limited Huffman by package-merge: optimal for a hard depth cap,
     and short enough to read. Returns a length per symbol (0 = unused). */
  function packageMerge(counts, maxLen) {
    const used = [];
    for (let sym = 0; sym < counts.length; sym++) if (counts[sym] > 0) used.push(sym);
    const lengths = new Uint8Array(counts.length);
    if (used.length === 0) return lengths;
    if (used.length === 1) { lengths[used[0]] = 1; return lengths; }

    const leaves = used.map((sym) => ({ weight: counts[sym], syms: [sym] }));
    leaves.sort((a, b) => a.weight - b.weight || a.syms[0] - b.syms[0]);

    let packages = [];
    for (let level = 0; level < maxLen; level++) {
      const merged = [];
      for (let i = 0; i + 1 < packages.length; i += 2) {
        merged.push({ weight: packages[i].weight + packages[i + 1].weight,
                      syms: packages[i].syms.concat(packages[i + 1].syms) });
      }
      packages = leaves.concat(merged).sort((a, b) => a.weight - b.weight);
    }
    const wanted = 2 * leaves.length - 2;
    for (let i = 0; i < wanted; i++) {
      for (const sym of packages[i].syms) lengths[sym]++;
    }
    return lengths;
  }

  /* Kraft check: a code is decodable exactly when the lengths sum to 1. */
  function kraftSum(lengths) {
    let space = 0;
    for (const len of lengths) if (len) space += Math.pow(2, -len);
    return space;
  }

  BM.huffman = {
    buildDecodeTable, readSymbol, buildEncodeTable, reverseBits,
    packageMerge, kraftSum,
  };
})(globalThis.BM || (globalThis.BM = {}));
