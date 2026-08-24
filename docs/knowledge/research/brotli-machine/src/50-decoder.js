/* 9.  The decoder.
   One pass over the bit stream, in the order the format defines it:
     stream header -> meta-block header -> prefix codes -> commands.
   Every read is wrapped so the page can replay it: `emit` records what was
   read, which bits it came from, and what changed in the machine's state. */
(function (BM) {
  "use strict";
  const T = BM.tables;
  const H = BM.huffman;

  const CTX_LUT = () => (BM._ctxLut || (BM._ctxLut = BM.base64ToBytes(BM.data.contextLutBase64)));

  class Output {
    constructor() { this.buf = new Uint8Array(1 << 16); this.length = 0; }
    _room(n) {
      if (this.length + n <= this.buf.length) return;
      let size = this.buf.length;
      while (size < this.length + n) size *= 2;
      const next = new Uint8Array(size);
      next.set(this.buf.subarray(0, this.length));
      this.buf = next;
    }
    push(byte) { this._room(1); this.buf[this.length++] = byte; }
    pushBytes(bytes) { this._room(bytes.length); this.buf.set(bytes, this.length); this.length += bytes.length; }
    copyBack(distance, length) {
      this._room(length);
      let src = this.length - distance;
      for (let i = 0; i < length; i++) this.buf[this.length + i] = this.buf[src + i];
      this.length += length;
    }
    bytes() { return this.buf.subarray(0, this.length); }
  }

  function decode(bytes, opts = {}) {
    const trace = opts.trace !== false;
    const maxEvents = opts.maxEvents ?? 400000;
    const reader = new BM.BitReader(bytes);
    const out = new Output();
    const events = [];
    const metablocks = [];
    const dict = BM.dictionary();
    let truncated = false;
    /* Always-on census: where the output bytes came from. Cheap enough to
       leave on, and it is the only way to ask a real stream what it used. */
    const counts = {
      metablocks: 0, commands: 0, literals: 0, copies: 0, copyBytes: 0,
      dictRefs: 0, dictBytes: 0, blockSwitches: 0, uncompressedBytes: 0,
      implicitDistances: 0, shortDistances: 0, cachedWords: new Map(),
      copyLengths: new Map(), dictLengths: new Map(),
      /* How far a full distance code was from the nearest cached distance.
         A copy that lands on the cache costs four bits; one that misses by
         three still does; beyond that the encoder pays a full symbol plus
         extra bits. */
      nearMiss: new Map(), distances: new Map(), fullDistances: 0,
    };
    const bump = (map, key) => map.set(key, (map.get(key) || 0) + 1);

    const emit = (kind, label, extra) => {
      if (!trace) return;
      if (events.length >= maxEvents) { truncated = true; return; }
      events.push(Object.assign({ kind, label, bit: reader.bitPos, out: out.length }, extra));
    };
    /* read + record in one move: the bit span of every field is kept. */
    const F = (kind, label, n, extra) => {
      const start = reader.bitPos;
      const value = reader.readBits(n);
      reader.map.push({ start, end: reader.bitPos, label, value, kind });
      if (trace && events.length < maxEvents) {
        events.push(Object.assign({ kind, label, value, bit0: start, bit1: reader.bitPos,
                                    out: out.length }, extra));
      }
      return value;
    };
    const sym = (kind, label, table, extra) => {
      const start = reader.bitPos;
      const value = H.readSymbol(reader, table);
      reader.map.push({ start, end: reader.bitPos, label, value, kind });
      if (trace && events.length < maxEvents) {
        events.push(Object.assign({ kind, label, value, bit0: start, bit1: reader.bitPos,
                                    out: out.length, bits: reader.bitPos - start }, extra));
      }
      return value;
    };

    /* --- 9.1 stream header ------------------------------------------- */
    let windowBits;
    {
      const start = reader.bitPos;
      if (reader.readBits(1) === 0) windowBits = 16;
      else {
        const n = reader.readBits(3);
        if (n !== 0) windowBits = 17 + n;
        else {
          const m = reader.readBits(3);
          if (m === 1) throw new BM.StreamError("large-window streams are out of scope here");
          windowBits = m !== 0 ? 8 + m : 17;
        }
      }
      reader.map.push({ start, end: reader.bitPos, label: "WBITS", value: windowBits, kind: "stream" });
      emit("stream", "WBITS", { value: windowBits, bit0: start, bit1: reader.bitPos,
        note: `window = 2^${windowBits} - 16 = ${((1 << windowBits) - 16).toLocaleString()} bytes` });
    }
    const maxBackwardDistance = (1 << windowBits) - T.WINDOW_GAP;

    /* --- 9.2 meta-blocks ---------------------------------------------- */
    let isLast = false;
    let guard = 0;
    while (!isLast) {
      if (++guard > 10000) throw new BM.StreamError("too many meta-blocks");
      const mbStartBit = reader.bitPos;
      const mb = { index: metablocks.length, startBit: mbStartBit, outStart: out.length };
      metablocks.push(mb);
      counts.metablocks++;

      isLast = F("mb", "ISLAST", 1) === 1;
      mb.isLast = isLast;
      if (isLast) {
        if (F("mb", "ISLASTEMPTY", 1) === 1) {
          mb.empty = true;
          mb.endBit = reader.bitPos;
          emit("mb", "empty last meta-block", {});
          break;
        }
      }
      const nibbles = F("mb", "MNIBBLES", 2) + 4;
      if (nibbles === 7) { /* value 3: metadata, not data */
        if (F("mb", "reserved", 1) !== 0) throw new BM.StreamError("reserved bit set");
        const skipBytes = F("mb", "MSKIPBYTES", 2);
        let skipLen = 0;
        for (let i = 0; i < skipBytes; i++) skipLen |= F("mb", `MSKIPLEN byte ${i}`, 8) << (i * 8);
        if (skipBytes) skipLen += 1;
        reader.skipToByteBoundary();
        for (let i = 0; i < skipLen; i++) reader.readBits(8);
        mb.metadata = true; mb.skipLen = skipLen; mb.endBit = reader.bitPos;
        emit("mb", "metadata meta-block skipped", { value: skipLen });
        continue;
      }
      let mlen = 0;
      for (let i = 0; i < nibbles; i++) mlen |= F("mb", `MLEN nibble ${i}`, 4) << (i * 4);
      mlen += 1;
      mb.mlen = mlen;
      let uncompressed = false;
      if (!isLast) uncompressed = F("mb", "ISUNCOMPRESSED", 1) === 1;
      mb.uncompressed = uncompressed;
      emit("mb", "meta-block length", { value: mlen,
        note: `${mlen} bytes${uncompressed ? ", stored uncompressed" : ""}` });

      if (uncompressed) {
        reader.skipToByteBoundary();
        counts.uncompressedBytes += mlen;
        for (let i = 0; i < mlen; i++) out.push(reader.readBits(8));
        mb.endBit = reader.bitPos;
        continue;
      }

      /* --- 9.2 block-type switching descriptors ----------------------- */
      const readVarLen = (label) => {
        const start = reader.bitPos;
        let value;
        if (reader.readBits(1) === 0) value = 0;
        else {
          const n = reader.readBits(3);
          value = n === 0 ? 1 : (1 << n) + reader.readBits(n);
        }
        value += 1;
        reader.map.push({ start, end: reader.bitPos, label, value, kind: "mb" });
        emit("mb", label, { value, bit0: start, bit1: reader.bitPos });
        return value;
      };
      const readBlockLength = (table, label) => {
        const code = sym("block", label + " code", table);
        const [base, extra] = T.BLOCK_LENGTH[code];
        const bits = extra ? F("block", label + " extra", extra) : 0;
        return base + bits;
      };

      const categories = ["literal", "insert&copy", "distance"];
      const blocks = [];
      for (let c = 0; c < 3; c++) {
        const count = readVarLen(`NBLTYPES ${categories[c]}`);
        const state = { count, type: 0, prevType: 1, length: 1 << 28, typeTable: null, lenTable: null };
        if (count >= 2) {
          state.typeTable = readPrefixCode(count + 2, `${categories[c]} block-type code`);
          state.lenTable = readPrefixCode(T.NUM_BLOCK_LENGTH_SYMBOLS, `${categories[c]} block-count code`);
          state.length = readBlockLength(state.lenTable, `${categories[c]} first block count`);
        }
        blocks.push(state);
      }
      mb.blocks = blocks.map((b) => ({ count: b.count, first: b.length }));

      /* --- 9.2 distance parameters ------------------------------------ */
      const npostfix = F("mb", "NPOSTFIX", 2);
      const ndirect = F("mb", "NDIRECT", 4) << npostfix;
      mb.npostfix = npostfix; mb.ndirect = ndirect;

      /* --- 7.1 context modes for each literal block type -------------- */
      const contextModes = [];
      for (let i = 0; i < blocks[0].count; i++) {
        contextModes.push(F("mb", `context mode, block type ${i}`, 2,
          { note: T.CONTEXT_MODES[reader.peekBits(0)] }));
      }
      mb.contextModes = contextModes.map((m) => T.CONTEXT_MODES[m]);

      /* --- 7.3 context maps ------------------------------------------- */
      const literalMap = readContextMap(blocks[0].count * 64, "literal context map");
      const distanceMap = readContextMap(blocks[2].count * 4, "distance context map");
      mb.numLiteralTrees = literalMap.numTrees;
      mb.numDistanceTrees = distanceMap.numTrees;

      /* --- 3.5 the prefix codes themselves ---------------------------- */
      const literalTrees = [];
      for (let i = 0; i < literalMap.numTrees; i++) {
        literalTrees.push(readPrefixCode(T.NUM_LITERAL_SYMBOLS, `literal code ${i}`));
      }
      const commandTrees = [];
      for (let i = 0; i < blocks[1].count; i++) {
        commandTrees.push(readPrefixCode(T.NUM_COMMAND_SYMBOLS, `insert&copy code ${i}`));
      }
      const distAlphabet = T.distanceAlphabetSize(npostfix, ndirect, 24);
      const distanceTrees = [];
      for (let i = 0; i < distanceMap.numTrees; i++) {
        distanceTrees.push(readPrefixCode(distAlphabet, `distance code ${i}`));
      }
      const distLut = T.distanceLut(npostfix, ndirect, distAlphabet);
      mb.headerEndBit = reader.bitPos;
      mb.headerBits = reader.bitPos - mbStartBit;
      emit("mb", "header complete", { note: `${mb.headerBits} bits of header` });

      /* --- 9.3 the command loop --------------------------------------- */
      const distCache = [16, 15, 11, 4];
      let distIdx = 0;
      let remaining = mlen;
      let commandIndex = 0;

      const switchBlock = (c) => {
        const state = blocks[c];
        const code = sym("block", `${categories[c]} block switch`, state.typeTable);
        let next;
        if (code === 0) next = state.prevType;
        else if (code === 1) next = state.type + 1;
        else next = code - 2;
        if (next >= state.count) next -= state.count;
        state.prevType = state.type;
        state.type = next;
        state.length = readBlockLength(state.lenTable, `${categories[c]} block count`);
        counts.blockSwitches++;
        emit("block", `${categories[c]} block type -> ${next}`, { value: next, note: `next ${state.length} elements` });
      };

      while (remaining > 0) {
        if (blocks[1].length === 0) switchBlock(1);
        blocks[1].length--;

        const cmdCode = sym("cmd", "insert&copy symbol", commandTrees[blocks[1].type], { commandIndex });
        const row = T.CMD_LUT[cmdCode];
        const insertExtra = row.insertExtra ? F("cmd", "insert extra bits", row.insertExtra) : 0;
        const copyExtra = row.copyExtra ? F("cmd", "copy extra bits", row.copyExtra) : 0;
        const insertLen = row.insertOffset + insertExtra;
        const copyLen = row.copyOffset + copyExtra;
        emit("cmd", "command", { value: cmdCode, insertLen, copyLen,
          implicitDistance: row.distanceCode === 0, commandIndex,
          note: `insert ${insertLen} literal${insertLen === 1 ? "" : "s"}, then copy ${copyLen}` });

        /* literals */
        for (let i = 0; i < insertLen; i++) {
          if (remaining === 0) break;
          if (blocks[0].length === 0) switchBlock(0);
          blocks[0].length--;
          const p1 = out.length >= 1 ? out.buf[out.length - 1] : 0;
          const p2 = out.length >= 2 ? out.buf[out.length - 2] : 0;
          const mode = contextModes[blocks[0].type];
          const lut = CTX_LUT();
          const context = lut[(mode << 9) + p1] | lut[(mode << 9) + 256 + p2];
          const treeIndex = literalMap.map[(blocks[0].type << 6) + context];
          const byte = sym("literal", "literal", literalTrees[treeIndex],
            { context, treeIndex, p1, p2, mode: T.CONTEXT_MODES[mode] });
          out.push(byte);
          counts.literals++;
          remaining--;
        }
        if (remaining === 0) { emit("cmd", "meta-block filled by literals", {}); break; }

        /* distance */
        let distance, distCodeUsed, dictRef = null;
        if (row.distanceCode === 0) {
          distance = distCache[(distIdx + 3) & 3];
          distCodeUsed = 0;
          counts.implicitDistances++;
          emit("dist", "implicit distance", { value: distance, note: "command code < 128 reuses the last distance" });
        } else {
          if (blocks[2].length === 0) switchBlock(2);
          blocks[2].length--;
          const distContext = row.distanceContext;
          const treeIndex = distanceMap.map[(blocks[2].type << 2) + distContext];
          const code = sym("dist", "distance symbol", distanceTrees[treeIndex], { distContext, treeIndex });
          distCodeUsed = code;
          if (code < 16) {
            counts.shortDistances++;
            const base = distCache[(distIdx + T.DIST_SHORT_INDEX[code]) & 3];
            distance = base + T.DIST_SHORT_DELTA[code];
            emit("dist", "short distance code", { value: code, distance,
              note: `${T.DIST_SHORT_NAME[code]} = ${distance}` });
          } else {
            const extra = distLut.extraBits[code];
            const bits = extra ? F("dist", "distance extra bits", extra) : 0;
            distance = distLut.offset[code] + (bits << npostfix);
            counts.fullDistances++;
            let best = Infinity;
            for (const cached of distCache) best = Math.min(best, Math.abs(distance - cached));
            const bucket = best === 0 ? "0" : best <= 3 ? "1-3" : best <= 16 ? "4-16"
              : best <= 64 ? "17-64" : best <= 256 ? "65-256" : best <= 4096 ? "257-4k" : ">4k";
            bump(counts.nearMiss, bucket);
            emit("dist", "distance", { value: code, distance,
              note: `offset ${distLut.offset[code]} + ${bits} << ${npostfix}` });
          }
          if (distance <= 0) throw new BM.StreamError("non-positive distance");
        }

        const maxDistance = Math.min(out.length, maxBackwardDistance);
        if (distance > maxDistance) {
          /* 8.  Not a copy: a static dictionary reference. */
          if (copyLen < 4 || copyLen > 24) throw new BM.StreamError(`dictionary copy length ${copyLen} out of range`);
          const id = distance - maxDistance - 1;
          const { wordIndex, transformIndex } = dict.decodeId(copyLen, id);
          if (transformIndex >= dict.transforms.length) throw new BM.StreamError("transform index out of range");
          const produced = dict.expand(copyLen, wordIndex, transformIndex);
          out.pushBytes(BM.bytesFromLatin1(produced));
          remaining -= produced.length;
          dictRef = { id, wordIndex, transformIndex, len: copyLen, produced };
          counts.dictRefs++;
          counts.dictBytes += produced.length;
          bump(counts.dictLengths, copyLen);
          bump(counts.cachedWords, produced);
          emit("dict", "dictionary reference", { value: id, distance, copyLen,
            word: dict.wordText(copyLen, wordIndex), transform: dict.describeTransform(transformIndex),
            produced, note: `word ${wordIndex} of length ${copyLen}, transform ${transformIndex}` });
        } else {
          if (distCodeUsed !== 0) { distCache[distIdx & 3] = distance; distIdx++; }
          bump(counts.distances, distance);
          out.copyBack(distance, copyLen);
          counts.copies++;
          counts.copyBytes += copyLen;
          bump(counts.copyLengths, copyLen);
          remaining -= copyLen;
          emit("copy", "copy", { distance, copyLen,
            text: BM.latin1(out.buf.subarray(out.length - copyLen, out.length)),
            cache: distCache.slice(), cacheIdx: distIdx,
            note: `copy ${copyLen} bytes from ${distance} back` });
        }
        commandIndex++;
        counts.commands++;
      }
      mb.endBit = reader.bitPos;
      mb.outEnd = out.length;
      mb.commands = commandIndex;
    }

    const padding = reader.bitPos & 7 ? 8 - (reader.bitPos & 7) : 0;
    return {
      output: out.bytes(), events, metablocks, map: reader.map, windowBits,
      maxBackwardDistance, bitsUsed: reader.bitPos, padding, truncated,
      inputBytes: bytes.length, counts,
    };

    /* --- 3.5 reading one prefix code --------------------------------- */
    function readPrefixCode(alphabetSize, label) {
      const start = reader.bitPos;
      const kind = F("code", `${label}: kind`, 2);
      const lengths = new Uint8Array(alphabetSize);
      let detail;
      if (kind === 1) {
        /* Simple code: 1..4 symbols listed literally. */
        const alphabetBits = Math.max(1, 32 - Math.clz32(alphabetSize - 1));
        const numSymbols = F("code", `${label}: NSYM-1`, 2) + 1;
        const symbols = [];
        for (let i = 0; i < numSymbols; i++) {
          symbols.push(F("code", `${label}: symbol ${i}`, alphabetBits));
        }
        if (new Set(symbols).size !== symbols.length) throw new BM.StreamError("repeated symbol in simple code");
        let treeSelect = 0;
        if (numSymbols === 4) treeSelect = F("code", `${label}: tree-select`, 1);
        /* 3.4.  Fixed length assignments; canonical ordering then sorts the
           equal-length symbols by value, which is what the C decoder does. */
        const assign = {
          1: [0],
          2: [1, 1],
          3: [1, 2, 2],
          4: treeSelect ? [1, 2, 3, 3] : [2, 2, 2, 2],
        }[numSymbols];
        symbols.forEach((s, i) => { lengths[s] = assign[i]; });
        detail = { simple: true, symbols, treeSelect, numSymbols };
      } else {
        /* Complex code: code lengths, themselves prefix coded. */
        const hskip = kind;
        const clLengths = new Uint8Array(T.CODE_LENGTH_CODES);
        let space = 32, numCodes = 0;
        for (let i = hskip; i < T.CODE_LENGTH_CODES; i++) {
          const idx = T.CODE_LENGTH_ORDER[i];
          const peek = reader.peekBits(4);
          const len = T.CL_PREFIX_LENGTH[peek];
          const v = T.CL_PREFIX_VALUE[peek];
          reader.readBits(len);
          clLengths[idx] = v;
          if (v !== 0) { space -= 32 >> v; numCodes++; if (space <= 0) break; }
        }
        if (!(numCodes === 1 || space === 0)) throw new BM.StreamError("code-length code is not complete");
        const clTable = H.buildDecodeTable(clLengths);
        /* Now the symbol lengths, with two repeat codes. */
        let prevLen = 8, repeat = 0, repeatLen = 0, sp = 32768, count = 0, i = 0;
        while (i < alphabetSize && sp > 0) {
          const s = H.readSymbol(reader, clTable);
          if (s < T.REPEAT_PREVIOUS) {
            lengths[i++] = s;
            repeat = 0;
            if (s !== 0) { prevLen = s; sp -= 32768 >> s; count++; }
            continue;
          }
          const extraBits = s === T.REPEAT_PREVIOUS ? 2 : 3;
          const newLen = s === T.REPEAT_PREVIOUS ? prevLen : 0;
          if (repeatLen !== newLen) { repeat = 0; repeatLen = newLen; }
          const oldRepeat = repeat;
          if (repeat > 0) { repeat -= 2; repeat <<= extraBits; }
          repeat += reader.readBits(extraBits) + 3;
          const times = repeat - oldRepeat;
          if (i + times > alphabetSize) throw new BM.StreamError("repeat runs past the alphabet");
          for (let k = 0; k < times; k++) lengths[i++] = newLen;
          if (newLen !== 0) { sp -= (32768 >> newLen) * times; count += times; }
        }
        if (sp !== 0 && count !== 1) throw new BM.StreamError("prefix code is not complete");
        detail = { simple: false, hskip, clLengths: Array.from(clLengths) };
      }
      const table = H.buildDecodeTable(lengths);
      const used = [];
      for (let s = 0; s < alphabetSize; s++) if (lengths[s]) used.push([s, lengths[s]]);
      if (detail.simple && detail.numSymbols === 1) { table.single = detail.symbols[0]; used.push([detail.symbols[0], 0]); }
      reader.map.push({ start, end: reader.bitPos, label, value: used.length, kind: "code" });
      emit("code", label, { bit0: start, bit1: reader.bitPos, detail,
        symbols: used.slice(0, 400), alphabetSize,
        note: `${used.length} symbols, ${reader.bitPos - start} bits` });
      table.info = { label, used, alphabetSize, detail, bits: reader.bitPos - start };
      return table;
    }

    /* --- 7.3 context map, with zero runs and move-to-front ------------ */
    function readContextMap(size, label) {
      const start = reader.bitPos;
      let numTrees;
      {
        const s0 = reader.bitPos;
        let v;
        if (reader.readBits(1) === 0) v = 0;
        else { const n = reader.readBits(3); v = n === 0 ? 1 : (1 << n) + reader.readBits(n); }
        numTrees = v + 1;
        reader.map.push({ start: s0, end: reader.bitPos, label: label + ": NTREES", value: numTrees, kind: "mb" });
      }
      const map = new Uint8Array(size);
      if (numTrees === 1) {
        emit("map", label, { value: 1, bit0: start, bit1: reader.bitPos, note: "one tree: every context maps to it" });
        return { map, numTrees, rlemax: 0, mtf: false };
      }
      let rlemax = 0;
      if (reader.peekBits(1) & 1) { reader.readBits(1); rlemax = reader.readBits(4) + 1; }
      else reader.readBits(1);
      const table = readPrefixCode(numTrees + rlemax, `${label}: code`);
      let i = 0;
      while (i < size) {
        const code = H.readSymbol(reader, table);
        if (code === 0) { map[i++] = 0; continue; }
        if (code > rlemax) { map[i++] = code - rlemax; continue; }
        const reps = (1 << code) + reader.readBits(code);
        if (i + reps > size) throw new BM.StreamError("context map run overflows");
        for (let k = 0; k < reps; k++) map[i++] = 0;
      }
      const mtf = reader.readBits(1) === 1;
      if (mtf) inverseMoveToFront(map);
      reader.map.push({ start, end: reader.bitPos, label, value: numTrees, kind: "map" });
      emit("map", label, { value: numTrees, bit0: start, bit1: reader.bitPos, rlemax, mtf,
        map: Array.from(map.subarray(0, Math.min(size, 512))),
        note: `${numTrees} trees over ${size} contexts${mtf ? ", move-to-front applied" : ""}` });
      return { map, numTrees, rlemax, mtf };
    }
  }

  function inverseMoveToFront(v) {
    const mtf = new Uint8Array(256);
    for (let i = 0; i < 256; i++) mtf[i] = i;
    for (let i = 0; i < v.length; i++) {
      const index = v[i];
      const value = mtf[index];
      v[i] = value;
      for (let j = index; j > 0; j--) mtf[j] = mtf[j - 1];
      mtf[0] = value;
    }
  }

  BM.decode = decode;
  BM.inverseMoveToFront = inverseMoveToFront;
})(globalThis.BM || (globalThis.BM = {}));
