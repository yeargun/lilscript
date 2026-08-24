/* The encoder.
   The format only constrains the decoder; everything an encoder does is a
   choice. This one makes the small set of choices that matter and leaves each
   of them as a replaceable function (see BM.plugins), so a different strategy
   can be dropped in and measured. Simplifications, all deliberate and all
   visible in the output: one block type per category (no block splitting),
   and the dictionary probe covers the identity / suffix / upper-case-first
   transform families rather than all 121. */
(function (BM) {
  "use strict";
  const T = BM.tables;
  const H = BM.huffman;

  const CTX_LUT = () => (BM._ctxLut || (BM._ctxLut = BM.base64ToBytes(BM.data.contextLutBase64)));

  /* --- code writing ------------------------------------------------- */

  /* A prefix code we can write: symbol -> {length, reversed}. When exactly one
     symbol is used, the format codes it in zero bits. */
  function makeWriter(lengths) {
    const table = H.buildEncodeTable(lengths);
    let used = 0, only = -1;
    for (let s = 0; s < lengths.length; s++) if (lengths[s]) { used++; only = s; }
    if (used === 1) table[only] = { length: 0, code: 0, reversed: 0 };
    return { table, used, only, lengths };
  }

  /* 3.4 / 3.5.  Write one prefix code: the cheap "simple" form for up to four
     symbols, otherwise the code-length sequence with a code of its own. */
  function writePrefixCode(w, lengths, alphabetSize, label) {
    const symbols = [];
    for (let s = 0; s < alphabetSize; s++) if (lengths[s]) symbols.push(s);
    const start = w.bitPos;
    if (symbols.length === 0) throw new Error(`${label}: no symbols to code`);
    if (symbols.length <= 4) {
      const alphabetBits = Math.max(1, 32 - Math.clz32(alphabetSize - 1));
      w.bits(`${label}: kind=simple`, 2, 1);
      w.bits(`${label}: NSYM-1`, 2, symbols.length - 1);
      /* Fixed length patterns; canonical order re-sorts equal lengths. */
      /* Write the symbols shortest-code-first: the reader assigns the fixed
         lengths positionally, so the order decides who gets the short code. */
      symbols.sort((a, b) => lengths[a] - lengths[b] || a - b);
      const flat = symbols.length === 4 && symbols.every((s) => lengths[s] === lengths[symbols[0]]);
      const treeSelect = symbols.length === 4 ? (flat ? 0 : 1) : 0;
      const shape = { 1: [0], 2: [1, 1], 3: [1, 2, 2],
                      4: treeSelect ? [1, 2, 3, 3] : [2, 2, 2, 2] }[symbols.length];
      const out = new Uint8Array(alphabetSize);
      symbols.forEach((s, i) => { out[s] = shape[i]; });
      for (let i = 0; i < symbols.length; i++) {
        w.bits(`${label}: symbol ${i}`, alphabetBits, symbols[i]);
      }
      if (symbols.length === 4) w.bits(`${label}: tree-select`, 1, treeSelect);
      w.map.push({ start, end: w.bitPos, label: `${label} (simple, ${symbols.length})`, kind: "code" });
      const writer = makeWriter(out);
      if (symbols.length === 1) writer.table[symbols[0]] = { length: 0, code: 0, reversed: 0 };
      return writer;
    }
    /* Complex code. Code lengths, run-length coded for zeros. */
    const clSymbols = [];
    for (let i = 0; i < alphabetSize;) {
      const len = lengths[i];
      if (len !== 0) { clSymbols.push({ sym: len }); i++; continue; }
      let run = 0;
      while (i + run < alphabetSize && lengths[i + run] === 0) run++;
      i += run;
      if (i >= alphabetSize) break; /* trailing zeros need not be coded */
      if (run < 3) { for (let k = 0; k < run; k++) clSymbols.push({ sym: 0 }); continue; }
      for (const piece of zeroRunCode(run)) clSymbols.push({ sym: T.REPEAT_ZERO, extra: piece, extraBits: 3 });
    }
    const clCounts = new Int32Array(T.CODE_LENGTH_CODES);
    for (const s of clSymbols) clCounts[s.sym]++;
    let clLengths = H.packageMerge(clCounts, 5);
    const clWriter = makeWriter(clLengths);
    /* HSKIP lets the first two or three entries of the fixed order be
       omitted; 1 is not encodable, because that pattern means "simple". */
    let zeros = 0;
    while (zeros < 3 && clLengths[T.CODE_LENGTH_ORDER[zeros]] === 0) zeros++;
    const hskip = zeros >= 3 ? 3 : zeros >= 2 ? 2 : 0;
    w.bits(`${label}: kind=complex`, 2, hskip);
    /* The reader stops as soon as the code-length code is complete, so the
       writer has to stop at exactly the same symbol. */
    let space = 32;
    for (let i = hskip; i < T.CODE_LENGTH_CODES; i++) {
      const idx = T.CODE_LENGTH_ORDER[i];
      const len = clLengths[idx];
      /* The fixed code for a code-length code length; see CL_PREFIX_*. */
      const pattern = { 0: [2, 0], 1: [4, 7], 2: [3, 3], 3: [2, 2], 4: [2, 1], 5: [4, 15] }[len];
      w.bits(`${label}: len[${idx}]=${len}`, pattern[0], pattern[1]);
      if (len !== 0) { space -= 32 >> len; if (space <= 0) break; }
    }
    for (const s of clSymbols) {
      w.field(`${label}: cl ${s.sym}`, () => {
        w.writeCode(clWriter.table[s.sym]);
        if (s.extraBits) w.writeBits(s.extraBits, s.extra);
      });
    }
    w.map.push({ start, end: w.bitPos, label: `${label} (complex, ${symbols.length} symbols)`, kind: "code" });
    return makeWriter(lengths);
  }

  /* A run of `run` zeros as a chain of REPEAT_ZERO symbols: each symbol
     multiplies the pending run by 8 (see 3.5). */
  function zeroRunCode(run) {
    if (run <= 10) return [run - 3];
    const extra = (run - 3) % 8;
    const prev = (run - 3 - extra) / 8 + 2;
    return [...zeroRunCode(prev), extra];
  }

  /* Write the "1 + n" length used by NBLTYPES / NTREES. */
  function writeVarLen(w, value, label) {
    const v = value - 1;
    w.field(label, () => {
      if (v === 0) { w.writeBits(1, 0); return; }
      w.writeBits(1, 1);
      if (v === 1) { w.writeBits(3, 0); return; }
      const n = 31 - Math.clz32(v); /* v >= 2 */
      w.writeBits(3, n);
      w.writeBits(n, v - (1 << n));
    }, value);
  }

  /* --- distance coding --------------------------------------------- */

  function distanceCoder(npostfix, ndirect) {
    const alphabetSize = T.distanceAlphabetSize(npostfix, ndirect, 24);
    const lut = T.distanceLut(npostfix, ndirect, alphabetSize);
    return {
      alphabetSize, lut,
      /* Cheapest way to say `distance`, given the cache. */
      encode(distance, cache, idx) {
        for (let code = 0; code < 16; code++) {
          const base = cache[(idx + T.DIST_SHORT_INDEX[code]) & 3];
          if (base + T.DIST_SHORT_DELTA[code] === distance) return { code, extra: 0, extraBits: 0 };
        }
        for (let code = 16; code < alphabetSize; code++) {
          const bits = lut.extraBits[code];
          const delta = distance - lut.offset[code];
          if (delta < 0) continue;
          if (delta & ((1 << npostfix) - 1)) continue;
          const extra = delta >> npostfix;
          if (extra < (1 << bits)) return { code, extra, extraBits: bits };
        }
        return null;
      },
    };
  }

  /* --- the pipeline ------------------------------------------------- */

  function encode(input, opts = {}) {
    const plugins = Object.assign({}, BM.plugins, opts.plugins || {});
    const trace = opts.trace !== false;
    const dict = BM.dictionary();
    const bytes = input instanceof Uint8Array ? input : BM.bytesFromLatin1(String(input));
    const text = BM.latin1(bytes);
    const stages = {};

    const params = plugins.chooseParams({ bytes, text, opts, BM });
    const w = new BM.BitWriter();

    /* 9.1 stream header. */
    writeWindowBits(w, params.windowBits);
    if (bytes.length === 0) {
      w.bits("ISLAST", 1, 1);
      w.bits("ISLASTEMPTY", 1, 1);
      return finish(w, bytes, { params, commands: [], stages, histograms: null });
    }
    if (bytes.length > (1 << 24)) throw new Error("this teaching encoder emits one meta-block; keep input under 16 MiB");

    const maxBackwardDistance = (1 << params.windowBits) - T.WINDOW_GAP;
    const ctx = { bytes, text, params, dict, maxBackwardDistance, BM, plugins, stages };

    /* 1. LZ77 + dictionary: turn the input into commands. */
    const commands = plugins.buildCommands(ctx);
    stages.commands = commands;

    /* 2. Histograms over the three alphabets, with literal contexts. */
    const hist = histograms(ctx, commands);
    stages.histograms = hist;

    /* 3. Cluster the 64 literal contexts into trees. */
    const clustering = plugins.clusterContexts(ctx, hist.literalByContext);
    stages.clustering = clustering;

    /* 4. Code lengths per alphabet. Every alphabet needs a code even when
       nothing used it — a stream with no back-references still declares a
       distance code — so an empty histogram gets one arbitrary symbol. */
    const nonEmpty = (counts) => {
      for (let i = 0; i < counts.length; i++) if (counts[i]) return counts;
      counts[0] = 1;
      return counts;
    };
    const literalLengths = clustering.histograms.map((h) => plugins.codeLengths(nonEmpty(h), 15));
    const commandLengths = plugins.codeLengths(nonEmpty(hist.command), 15);
    const dcoder = distanceCoder(params.npostfix, params.ndirect);
    const distanceLengths = plugins.codeLengths(nonEmpty(hist.distance), 15);
    stages.codeLengths = { literalLengths, commandLengths, distanceLengths };

    /* 5. Serialize. */
    w.field("ISLAST", () => w.writeBits(1, 1), 1);
    w.field("ISLASTEMPTY", () => w.writeBits(1, 0), 0);
    const mlen = bytes.length;
    const nibbles = mlen - 1 < (1 << 16) ? 4 : mlen - 1 < (1 << 20) ? 5 : 6;
    w.bits("MNIBBLES", 2, nibbles - 4);
    for (let i = 0; i < nibbles; i++) w.bits(`MLEN nibble ${i}`, 4, ((mlen - 1) >> (i * 4)) & 0xf);

    writeVarLen(w, 1, "NBLTYPES literal");
    writeVarLen(w, 1, "NBLTYPES insert&copy");
    writeVarLen(w, 1, "NBLTYPES distance");
    w.bits("NPOSTFIX", 2, params.npostfix);
    w.bits("NDIRECT", 4, params.ndirect >> params.npostfix);
    w.bits("context mode", 2, params.contextMode);

    /* 7.3 literal context map. */
    writeVarLen(w, clustering.numTrees, "NTREESL");
    if (clustering.numTrees > 1) {
      w.bits("RLEMAX=0", 1, 0);
      const mapCounts = new Int32Array(clustering.numTrees);
      for (const t of clustering.map) mapCounts[t]++;
      const mapLengths = plugins.codeLengths(mapCounts, 15);
      const mapWriter = writePrefixCode(w, mapLengths, clustering.numTrees, "context map code");
      for (let i = 0; i < clustering.map.length; i++) {
        w.field(`context ${i} -> tree ${clustering.map[i]}`, () => w.writeCode(mapWriter.table[clustering.map[i]]));
      }
      w.bits("IMTF", 1, 0);
    }
    writeVarLen(w, 1, "NTREESD");

    const literalWriters = literalLengths.map((lengths, i) =>
      writePrefixCode(w, lengths, T.NUM_LITERAL_SYMBOLS, `literal code ${i}`));
    const commandWriter = writePrefixCode(w, commandLengths, T.NUM_COMMAND_SYMBOLS, "insert&copy code");
    const distanceWriter = writePrefixCode(w, distanceLengths, dcoder.alphabetSize, "distance code");
    const headerBits = w.bitPos;

    /* 9.3 the commands. */
    const cache = [16, 15, 11, 4];
    let idx = 0;
    let pos = 0;
    const lut = CTX_LUT();
    const events = [];
    for (const cmd of commands) {
      const symbol = cmd.symbol;
      const row = T.CMD_LUT[symbol];
      const cmdStart = w.bitPos;
      w.field(`cmd ${symbol}`, () => {
        w.writeCode(commandWriter.table[symbol]);
        if (row.insertExtra) w.writeBits(row.insertExtra, cmd.insertLen - row.insertOffset);
        if (row.copyExtra) w.writeBits(row.copyExtra, cmd.copyLen - row.copyOffset);
      }, symbol);
      for (let i = 0; i < cmd.insertLen; i++) {
        const p = pos + i;
        const p1 = p >= 1 ? bytes[p - 1] : 0;
        const p2 = p >= 2 ? bytes[p - 2] : 0;
        const context = lut[(params.contextMode << 9) + p1] | lut[(params.contextMode << 9) + 256 + p2];
        const tree = clustering.map[context];
        w.field(`literal ${JSON.stringify(text[p])}`, () => w.writeCode(literalWriters[tree].table[bytes[p]]));
      }
      pos += cmd.insertLen;
      if (cmd.kind === "end") { pos += 0; break; }
      if (cmd.distanceCode !== 0) {
        const d = dcoder.encode(cmd.distance, cache, idx);
        w.field(`distance ${cmd.distance}`, () => {
          w.writeCode(distanceWriter.table[d.code]);
          if (d.extraBits) w.writeBits(d.extraBits, d.extra);
        }, d.code);
        if (d.code !== 0 && !cmd.dictionary) { cache[idx & 3] = cmd.distance; idx++; }
      }
      pos += cmd.dictionary ? cmd.dictionary.produced.length : cmd.copyLen;
      if (trace) events.push({ cmd, bits: w.bitPos - cmdStart, pos });
    }

    return finish(w, bytes, { params, commands, stages, histograms: hist, headerBits,
                              clustering, events, dcoder });
  }

  function finish(w, input, extra) {
    const bytes = w.finish();
    return Object.assign({
      bytes, map: w.map, bits: w.bitPos, input,
      ratio: input.length ? bytes.length / input.length : 0,
    }, extra);
  }

  function writeWindowBits(w, windowBits) {
    w.field("WBITS", () => {
      if (windowBits === 16) { w.writeBits(1, 0); return; }
      if (windowBits >= 18 && windowBits <= 24) { w.writeBits(1, 1); w.writeBits(3, windowBits - 17); return; }
      if (windowBits === 17) { w.writeBits(1, 1); w.writeBits(3, 0); w.writeBits(3, 0); return; }
      if (windowBits >= 10 && windowBits <= 16) {
        w.writeBits(1, 1); w.writeBits(3, 0); w.writeBits(3, windowBits - 8); return;
      }
      throw new Error(`window bits ${windowBits} out of range`);
    }, windowBits);
  }

  /* Histograms, including the per-context literal split. */
  function histograms(ctx, commands) {
    const { bytes, params } = ctx;
    const lut = CTX_LUT();
    const literalByContext = Array.from({ length: 64 }, () => new Int32Array(256));
    const command = new Int32Array(T.NUM_COMMAND_SYMBOLS);
    const dcoder = distanceCoder(params.npostfix, params.ndirect);
    const distance = new Int32Array(dcoder.alphabetSize);
    const cache = [16, 15, 11, 4];
    let idx = 0, pos = 0;
    for (const cmd of commands) {
      command[cmd.symbol]++;
      for (let i = 0; i < cmd.insertLen; i++) {
        const p = pos + i;
        const p1 = p >= 1 ? bytes[p - 1] : 0;
        const p2 = p >= 2 ? bytes[p - 2] : 0;
        const context = lut[(params.contextMode << 9) + p1] | lut[(params.contextMode << 9) + 256 + p2];
        literalByContext[context][bytes[p]]++;
      }
      pos += cmd.insertLen;
      if (cmd.kind === "end") break;
      if (cmd.distanceCode !== 0) {
        const d = dcoder.encode(cmd.distance, cache, idx);
        if (!d) throw new Error(`distance ${cmd.distance} is not encodable`);
        cmd.distanceSymbol = d.code;
        distance[d.code]++;
        if (d.code !== 0 && !cmd.dictionary) { cache[idx & 3] = cmd.distance; idx++; }
      }
      pos += cmd.dictionary ? cmd.dictionary.produced.length : cmd.copyLen;
    }
    const literal = new Int32Array(256);
    for (const h of literalByContext) for (let i = 0; i < 256; i++) literal[i] += h[i];
    return { literal, literalByContext, command, distance };
  }

  /* Try several parameter sets and keep the smallest stream. */
  function encodeBest(input, opts = {}) {
    const trials = opts.trials || [
      { label: "1 literal tree", literalTrees: 1 },
      { label: "2 literal trees", literalTrees: 2 },
      { label: "4 literal trees", literalTrees: 4 },
      { label: "8 literal trees", literalTrees: 8 },
    ];
    const modes = opts.modes || [2, 0];
    const results = [];
    let best = null;
    for (const trial of trials) {
      for (const mode of modes) {
        let r;
        try {
          r = encode(input, Object.assign({}, opts, {
            trace: false,
            plugins: Object.assign({}, opts.plugins, {
              chooseParams: (c) => Object.assign(BM.plugins.chooseParams(c), {
                contextMode: mode, literalTrees: trial.literalTrees,
              }),
            }),
          }));
        } catch (e) { results.push({ label: trial.label, mode, error: e.message }); continue; }
        results.push({ label: trial.label, mode: T.CONTEXT_MODES[mode], size: r.bytes.length,
                       trees: r.clustering ? r.clustering.numTrees : 1 });
        if (!best || r.bytes.length < best.result.bytes.length) {
          best = { result: r, trial, mode };
        }
      }
    }
    return { best, results };
  }

  BM.encode = encode;
  BM.encodeBest = encodeBest;
  BM.encoderInternals = { makeWriter, writePrefixCode, zeroRunCode, distanceCoder, histograms, writeVarLen };
})(globalThis.BM || (globalThis.BM = {}));
