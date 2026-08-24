/* The encoder's replaceable parts.
   Each function here is a decision the format leaves open. The page shows the
   source of every one of them in an editor; replacing one and re-running is
   the whole point of this exercise. Keep the signatures. */
(function (BM) {
  "use strict";
  const T = BM.tables;

  const plugins = {
    /* Window size, distance parameters, context mode, literal-tree budget. */
    chooseParams: function chooseParams(ctx) {
      const n = ctx.bytes.length;
      const need = Math.max(10, 32 - Math.clz32(Math.max(1, n + T.WINDOW_GAP - 1)));
      return {
        windowBits: Math.min(24, Math.max(16, need)),
        npostfix: 0,
        ndirect: 0,
        contextMode: 2,          /* 0 LSB6, 1 MSB6, 2 UTF8, 3 SIGNED */
        literalTrees: n >= 512 ? 4 : 1,
        minMatch: 4,
        chainLength: 64,         /* how many hash-chain candidates to test */
        maxMatch: 1024,
        lazy: true,
        useDictionary: true,
        literalBits: 7,          /* rough cost of a literal, for match scoring */
      };
    },

    /* Hash chains over 4-byte prefixes: head[hash] plus a "previous position
       with this hash" link per byte. */
    buildMatchIndex: function buildMatchIndex(ctx) {
      const { bytes } = ctx;
      const bits = 17;
      const head = new Int32Array(1 << bits).fill(-1);
      const prev = new Int32Array(bytes.length).fill(-1);
      const hashAt = (p) => {
        const v = (bytes[p] | (bytes[p + 1] << 8) | (bytes[p + 2] << 16) | (bytes[p + 3] << 24)) >>> 0;
        return (Math.imul(v, 0x1e35a7bd) >>> (32 - bits));
      };
      return {
        head, prev, hashAt,
        insert(p) {
          if (p + 4 > bytes.length) return;
          const h = hashAt(p);
          prev[p] = head[h];
          head[h] = p;
        },
      };
    },

    /* Longest back-reference at `pos`, preferring distances already in the
       cache (they cost four bits instead of twenty). */
    findMatch: function findMatch(ctx, index, pos, cache, cacheIdx) {
      const { bytes, params } = ctx;
      const n = bytes.length;
      if (pos + params.minMatch > n) return null;
      const maxDistance = Math.min(pos, ctx.maxBackwardDistance);
      const limit = Math.min(n - pos, params.maxMatch);
      let best = null;
      let candidate = index.head[index.hashAt(pos)];
      let tries = params.chainLength;
      while (candidate >= 0 && tries-- > 0) {
        const distance = pos - candidate;
        if (distance > maxDistance || distance <= 0) break;
        let len = 0;
        while (len < limit && bytes[candidate + len] === bytes[pos + len]) len++;
        if (len >= params.minMatch) {
          const inCache = cache.some((d) => d === distance);
          const score = len * params.literalBits - matchCost(distance, inCache);
          if (!best || score > best.score) best = { kind: "copy", len, distance, score, inCache };
        }
        candidate = index.prev[candidate];
      }
      return best;
    },

    /* Static dictionary probe: the longest word that spells the text here,
       then the suffix transforms that extend it, then upper-case-first.
       (Prefix and omit-first families are left as an exercise.) */
    dictProbe: function dictProbe(ctx, pos) {
      const { text, dict, params } = ctx;
      if (!params.useDictionary) return null;
      const exact = dict.exactIndex();
      const suffixes = dictSuffixTransforms(dict);
      const maxLen = Math.min(24, text.length - pos);
      let best = null;
      const consider = (len, wordIndex, transform, produced) => {
        if (text.substr(pos, produced.length) !== produced) return;
        const id = dict.entryId(len, wordIndex, transform);
        const distance = Math.min(pos, ctx.maxBackwardDistance) + 1 + id;
        const score = produced.length * params.literalBits - matchCost(distance, false);
        if (!best || score > best.score) {
          best = { kind: "dictionary", len, wordIndex, transform, produced, distance, score,
                   word: dict.wordText(len, wordIndex) };
        }
      };
      for (let len = maxLen; len >= 4; len--) {
        if (!dict.countFor(len)) continue;
        const key = text.substr(pos, len);
        const wordIndex = exact.get(len).get(key);
        if (wordIndex !== undefined) {
          consider(len, wordIndex, 0, key);
          for (const [t, suffix] of suffixes) {
            if (text.substr(pos + len, suffix.length) === suffix) consider(len, wordIndex, t, key + suffix);
          }
          break; /* longest word wins; shorter ones rarely beat it */
        }
      }
      const first = text.charCodeAt(pos);
      if (first >= 65 && first <= 90) {
        const lower = String.fromCharCode(first | 32);
        for (let len = maxLen; len >= 4; len--) {
          if (!dict.countFor(len)) continue;
          const key = lower + text.substr(pos + 1, len - 1);
          const wordIndex = exact.get(len).get(key);
          if (wordIndex !== undefined) {
            for (const t of upperFirstTransforms(dict)) {
              consider(len, wordIndex, t, dict.applyTransform(key, t));
            }
            break;
          }
        }
      }
      return best;
    },

    /* Walk the input, choose matches, and turn the choices into commands. */
    buildCommands: function buildCommands(ctx) {
      const { bytes, params, plugins } = ctx;
      const n = bytes.length;
      const index = plugins.buildMatchIndex(ctx);
      const commands = [];
      const cache = [16, 15, 11, 4];
      let cacheIdx = 0;
      let pos = 0;
      let insertStart = 0;

      const emit = (matchPos, chosen) => {
        const insertLen = matchPos - insertStart;
        const copyLen = chosen.kind === "dictionary" ? chosen.len : chosen.len;
        const reusesLast = chosen.kind !== "dictionary" && chosen.distance === cache[(cacheIdx + 3) & 3];
        /* An implicit-distance command (code < 128) exists only for short
           inserts and copies; fall back to spelling the distance out. */
        let pick = reusesLast ? T.commandSymbol(insertLen, copyLen, true) : null;
        if (!pick) pick = T.commandSymbol(insertLen, copyLen, false);
        const implicit = T.CMD_LUT[pick.code].distanceCode === 0;
        const cmd = {
          kind: chosen.kind, symbol: pick.code, insertLen, copyLen,
          distance: chosen.distance, distanceCode: implicit ? 0 : 1,
          literals: insertStart, produced: chosen.kind === "dictionary" ? chosen.produced.length : chosen.len,
        };
        if (chosen.kind === "dictionary") {
          cmd.dictionary = { wordIndex: chosen.wordIndex, transform: chosen.transform,
                             produced: chosen.produced, word: chosen.word, len: chosen.len };
        }
        commands.push(cmd);
        /* Mirror the decoder exactly: the cache moves on unless the distance
           was the last one (implicitly or through short code 0). */
        if (!implicit && !reusesLast && chosen.kind !== "dictionary") {
          cache[cacheIdx & 3] = chosen.distance;
          cacheIdx++;
        }
        return cmd;
      };

      while (pos < n) {
        let chosen = plugins.findMatch(ctx, index, pos, cache, cacheIdx);
        const dictMatch = plugins.dictProbe(ctx, pos);
        if (dictMatch && (!chosen || dictMatch.score > chosen.score)) chosen = dictMatch;

        if (chosen && chosen.score > 0 && params.lazy && pos + 1 < n) {
          /* Lazy matching: one more literal may buy a better match. */
          const next = plugins.findMatch(ctx, index, pos + 1, cache, cacheIdx);
          const nextDict = plugins.dictProbe(ctx, pos + 1);
          const better = nextDict && (!next || nextDict.score > next.score) ? nextDict : next;
          if (better && better.score > chosen.score + params.literalBits) chosen = null;
        }

        if (!chosen || chosen.score <= 0) {
          index.insert(pos);
          pos++;
          continue;
        }
        emit(pos, chosen);
        const advance = chosen.kind === "dictionary" ? chosen.produced.length : chosen.len;
        for (let i = 0; i < advance && pos + i < n; i++) index.insert(pos + i);
        pos += advance;
        insertStart = pos;
      }

      if (insertStart < n) {
        /* Trailing literals: the meta-block ends inside the insert, so the
           copy this command declares is never performed. */
        const insertLen = n - insertStart;
        const pick = T.commandSymbol(insertLen, 2, false);
        commands.push({ kind: "end", symbol: pick.code, insertLen, copyLen: 2,
                        distance: 0, distanceCode: 0, literals: insertStart, produced: 0 });
      }
      return commands;
    },

    /* Group the 64 literal contexts into a few trees (Lloyd's algorithm with
       a cross-entropy cost). One tree per context would cost more in headers
       than it saves in symbols. */
    clusterContexts: function clusterContexts(ctx, byContext) {
      const k = Math.max(1, Math.min(64, ctx.params.literalTrees | 0));
      const totals = byContext.map((h) => h.reduce((a, b) => a + b, 0));
      const combined = new Int32Array(256);
      for (const h of byContext) for (let i = 0; i < 256; i++) combined[i] += h[i];
      if (k === 1) {
        return { map: new Uint8Array(64), numTrees: 1, histograms: [combined], iterations: 0 };
      }
      /* Seed with the k busiest contexts. */
      const order = totals.map((t, i) => [t, i]).sort((a, b) => b[0] - a[0]);
      let centers = order.slice(0, k).filter(([t]) => t > 0).map(([, i]) => Int32Array.from(byContext[i]));
      if (centers.length === 0) centers = [combined];
      let map = new Uint8Array(64);
      let iterations = 0;
      for (let iter = 0; iter < 8; iter++) {
        iterations++;
        const models = centers.map(logModel);
        let moved = false;
        for (let c = 0; c < 64; c++) {
          if (totals[c] === 0) continue;
          let bestCost = Infinity, bestIdx = 0;
          for (let m = 0; m < models.length; m++) {
            const cost = crossEntropy(byContext[c], models[m]);
            if (cost < bestCost) { bestCost = cost; bestIdx = m; }
          }
          if (map[c] !== bestIdx) { map[c] = bestIdx; moved = true; }
        }
        const next = Array.from({ length: centers.length }, () => new Int32Array(256));
        for (let c = 0; c < 64; c++) {
          if (totals[c] === 0) continue;
          const h = byContext[c];
          for (let i = 0; i < 256; i++) next[map[c]][i] += h[i];
        }
        centers = next;
        if (!moved && iter > 0) break;
      }
      /* Drop clusters nothing landed in, and renumber. */
      const keep = [];
      const remap = new Int32Array(centers.length).fill(-1);
      for (let m = 0; m < centers.length; m++) {
        let total = 0;
        for (let i = 0; i < 256; i++) total += centers[m][i];
        if (total > 0) { remap[m] = keep.length; keep.push(centers[m]); }
      }
      if (keep.length === 0) { keep.push(combined); remap.fill(0); }
      const finalMap = new Uint8Array(64);
      for (let c = 0; c < 64; c++) finalMap[c] = Math.max(0, remap[map[c]]);
      return { map: finalMap, numTrees: keep.length, histograms: keep, iterations };
    },

    /* Counts -> code lengths, capped at 15 bits. */
    codeLengths: function codeLengths(counts, maxLen) {
      return BM.huffman.packageMerge(counts, maxLen);
    },
  };

  function matchCost(distance, inCache) {
    if (inCache) return 16;
    const distBits = Math.max(1, 32 - Math.clz32(distance)) ;
    return 12 + distBits;
  }
  function logModel(hist) {
    let total = 0;
    for (let i = 0; i < 256; i++) total += hist[i];
    const model = new Float64Array(256);
    const eps = 0.02;
    for (let i = 0; i < 256; i++) model[i] = -Math.log2((hist[i] + eps) / (total + eps * 256));
    return model;
  }
  function crossEntropy(hist, model) {
    let cost = 0;
    for (let i = 0; i < 256; i++) if (hist[i]) cost += hist[i] * model[i];
    return cost;
  }
  let _suffixCache = null, _upperCache = null;
  function dictSuffixTransforms(dict) {
    if (_suffixCache) return _suffixCache;
    _suffixCache = [];
    for (let t = 0; t < dict.transforms.length; t++) {
      const { prefix, type, suffix } = dict.transformParts(t);
      if (prefix === "" && type === 0 && suffix !== "") _suffixCache.push([t, suffix]);
    }
    return _suffixCache;
  }
  function upperFirstTransforms(dict) {
    if (_upperCache) return _upperCache;
    _upperCache = [];
    for (let t = 0; t < dict.transforms.length; t++) {
      const { prefix, type } = dict.transformParts(t);
      if (prefix === "" && type === 10) _upperCache.push(t);
    }
    return _upperCache;
  }

  BM.plugins = plugins;
  BM.defaultPlugins = Object.assign({}, plugins);
  BM.pluginHelpers = { matchCost, logModel, crossEntropy, dictSuffixTransforms, upperFirstTransforms };
})(globalThis.BM || (globalThis.BM = {}));
