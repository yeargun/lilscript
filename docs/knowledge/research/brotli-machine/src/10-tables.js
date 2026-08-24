/* RFC 7932 static tables.
   Everything here is derived from the two 24-entry length tables and the
   26-entry block-length table; the build proves the derivation against the
   kCmdLut that ships with the C decoder. */
(function (BM) {
  "use strict";
  const T = (BM.tables = {});

  /* 5.  Insert and copy lengths: base value + number of extra bits. */
  T.INSERT_LENGTH = [
    [0, 0], [1, 0], [2, 0], [3, 0], [4, 0], [5, 0], [6, 1], [8, 1],
    [10, 2], [14, 2], [18, 3], [26, 3], [34, 4], [50, 4], [66, 5], [98, 5],
    [130, 6], [194, 7], [322, 8], [578, 9], [1090, 10], [2114, 12],
    [6210, 14], [22594, 24],
  ];
  T.COPY_LENGTH = [
    [2, 0], [3, 0], [4, 0], [5, 0], [6, 0], [7, 0], [8, 0], [9, 0],
    [10, 1], [12, 1], [14, 2], [18, 2], [22, 3], [30, 3], [38, 4], [54, 4],
    [70, 5], [102, 5], [134, 6], [198, 7], [326, 8], [582, 9], [1094, 10],
    [2118, 24],
  ];
  /* 9.2.  Block counts share one 26-symbol alphabet. */
  T.BLOCK_LENGTH = [
    [1, 2], [5, 2], [9, 2], [13, 2], [17, 3], [25, 3], [33, 3], [41, 3],
    [49, 4], [65, 4], [81, 4], [97, 4], [113, 5], [145, 5], [177, 5], [209, 5],
    [241, 6], [305, 6], [369, 7], [497, 8], [753, 9], [1265, 10], [2289, 11],
    [4337, 12], [8433, 13], [16625, 24],
  ];

  /* 4.  The 16 short distance codes read the last-distance cache. */
  T.DIST_SHORT_INDEX = [3, 2, 1, 0, 3, 3, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2];
  T.DIST_SHORT_DELTA = [0, 0, 0, 0, -1, 1, -2, 2, -3, 3, -1, 1, -2, 2, -3, 3];
  T.DIST_SHORT_NAME = [
    "last", "2nd last", "3rd last", "4th last",
    "last-1", "last+1", "last-2", "last+2", "last-3", "last+3",
    "2nd-1", "2nd+1", "2nd-2", "2nd+2", "2nd-3", "2nd+3",
  ];

  /* 3.5.  Complex prefix codes: the order code lengths arrive in, and the
     fixed 4-bit lookup that decodes each code-length code length. */
  T.CODE_LENGTH_ORDER = [1, 2, 3, 4, 0, 5, 17, 6, 16, 7, 8, 9, 10, 11, 12, 13, 14, 15];
  T.CL_PREFIX_LENGTH = [2, 2, 2, 3, 2, 2, 2, 4, 2, 2, 2, 3, 2, 2, 2, 4];
  T.CL_PREFIX_VALUE = [0, 4, 3, 2, 0, 4, 3, 1, 0, 4, 3, 2, 0, 4, 3, 5];
  T.REPEAT_PREVIOUS = 16;
  T.REPEAT_ZERO = 17;
  T.CODE_LENGTH_CODES = 18;

  T.NUM_LITERAL_SYMBOLS = 256;
  T.NUM_COMMAND_SYMBOLS = 704;
  T.NUM_BLOCK_LENGTH_SYMBOLS = 26;
  T.NUM_DISTANCE_SHORT_CODES = 16;
  T.MAX_CODE_LENGTH = 15;
  T.WINDOW_GAP = 16;
  T.CONTEXT_MODES = ["LSB6", "MSB6", "UTF8", "SIGNED"];

  /* 5.  The 704 insert-and-copy symbols are a product of an 8x8 grid of
     (insert bucket, copy bucket) pairs across 11 range blocks; the block
     decides whether the distance is coded (0..127 reuse the last distance)
     and which sub-range of the two length alphabets applies. */
  function buildCommandLut() {
    const rows = [];
    for (let code = 0; code < 704; code++) {
      /* Which of the 11 (insert-range, copy-range) blocks the symbol is in. */
      const rangeIdx = code >> 6;
      const insertRange = [0, 0, 0, 0, 8, 8, 0, 16, 8, 16, 16][rangeIdx];
      const copyRange = [0, 8, 0, 8, 0, 8, 16, 0, 16, 8, 16][rangeIdx];
      /* Codes 0..127 imply "distance = last distance" and code no distance. */
      const distanceCode = code < 128 ? 0 : -1;
      const sub = code & 0x3f;
      const insertCode = insertRange + (sub >> 3);
      const copyCode = copyRange + (sub & 7);
      const [insertOffset, insertExtra] = T.INSERT_LENGTH[insertCode];
      const [copyOffset, copyExtra] = T.COPY_LENGTH[copyCode];
      /* 7.3.  The distance context of the command is its copy length,
         clamped: 2 -> 0, 3 -> 1, 4 -> 2, longer -> 3. */
      const context = Math.min(copyCode, 3);
      rows.push({
        code, insertCode, copyCode, insertOffset, insertExtra,
        copyOffset, copyExtra, distanceCode, distanceContext: context, context,
      });
    }
    return rows;
  }
  T.CMD_LUT = buildCommandLut();

  /* Inverse map: pick the cheapest symbol for a concrete (insert, copy, dist0). */
  T.commandSymbol = function (insertLen, copyLen, useLastDistance) {
    let insertCode = 0;
    while (insertCode + 1 < 24 && T.INSERT_LENGTH[insertCode + 1][0] <= insertLen) insertCode++;
    let copyCode = 0;
    while (copyCode + 1 < 24 && T.COPY_LENGTH[copyCode + 1][0] <= copyLen) copyCode++;
    /* Find the range block that covers this pair with the right distance rule. */
    for (let rangeIdx = 0; rangeIdx < 11; rangeIdx++) {
      const insertRange = [0, 0, 0, 0, 8, 8, 0, 16, 8, 16, 16][rangeIdx];
      const copyRange = [0, 8, 0, 8, 0, 8, 16, 0, 16, 8, 16][rangeIdx];
      const wantsImplicit = rangeIdx < 2;
      if (wantsImplicit !== !!useLastDistance) continue;
      if (insertCode < insertRange || insertCode >= insertRange + 8) continue;
      if (copyCode < copyRange || copyCode >= copyRange + 8) continue;
      const code = (rangeIdx << 6) | ((insertCode - insertRange) << 3) | (copyCode - copyRange);
      return { code, insertCode, copyCode };
    }
    return null;
  };

  /* 4.  Distance alphabet layout for a given NPOSTFIX / NDIRECT. */
  T.distanceLut = function (npostfix, ndirect, alphabetSize) {
    const extraBits = new Uint8Array(alphabetSize);
    const offset = new Int32Array(alphabetSize);
    let i = T.NUM_DISTANCE_SHORT_CODES;
    for (let j = 0; j < ndirect; j++) { extraBits[i] = 0; offset[i] = j + 1; i++; }
    const postfix = 1 << npostfix;
    let bits = 1, half = 0;
    while (i < alphabetSize) {
      const base = ndirect + ((((2 + half) << bits) - 4) << npostfix) + 1;
      for (let j = 0; j < postfix && i < alphabetSize; j++) {
        extraBits[i] = bits; offset[i] = base + j; i++;
      }
      bits += half; half ^= 1;
    }
    return { extraBits, offset };
  };

  T.distanceAlphabetSize = function (npostfix, ndirect, maxBits) {
    return T.NUM_DISTANCE_SHORT_CODES + ndirect + (maxBits << (npostfix + 1));
  };
})(globalThis.BM || (globalThis.BM = {}));
