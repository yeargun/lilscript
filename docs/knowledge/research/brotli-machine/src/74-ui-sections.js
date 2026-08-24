/* Sections 03–09: the reference views, each one reading the live stream. */
(function (BM) {
  "use strict";
  const U = BM.ui;
  const T = BM.tables;
  const lut = () => (BM._ctxLut || (BM._ctxLut = BM.base64ToBytes(BM.data.contextLutBase64)));

  /* --- 03 prefix codes ------------------------------------------------ */
  function initCodes() {
    U.$("#cl-order").textContent = T.CODE_LENGTH_ORDER.join(" ");
    const run = () => {
      const raw = U.$("#huff-input").value;
      const pairs = raw.split(/[,\n]/).map((s) => s.trim()).filter(Boolean).map((s) => {
        const m = /^(.*?)[\s:=]+(\d+)$/.exec(s);
        return m ? [m[1].trim(), Number(m[2])] : [s, 1];
      });
      if (!pairs.length) return;
      const counts = new Int32Array(pairs.length);
      pairs.forEach(([, c], i) => { counts[i] = c; });
      const lengths = BM.huffman.packageMerge(counts, 15);
      const codes = BM.huffman.buildEncodeTable(lengths);
      const total = pairs.reduce((a, [, c]) => a + c, 0);
      let bits = 0, entropy = 0;
      pairs.forEach(([, c], i) => {
        bits += c * lengths[i];
        if (c) entropy -= c * Math.log2(c / total);
      });
      U.$("#huff-summary").textContent =
        `${bits} bits with this code · ${entropy.toFixed(1)} bits is the entropy floor · Kraft ${BM.huffman.kraftSum(lengths).toFixed(3)}`;
      const built = U.table(
        [{ text: "symbol" }, { text: "count", num: true }, { text: "length", num: true },
         { text: "code" }, { text: "written" }],
        pairs.map(([label, count], i) => [
          label, { text: U.num(count), num: true }, { text: String(lengths[i]), num: true },
          codes[i] ? BM.bin(codes[i].code, lengths[i]) : "—",
          codes[i] ? BM.bin(codes[i].reversed, lengths[i]) : "—",
        ]));
      const host = U.$("#huff-table");
      host.replaceWith(built);
      built.id = "huff-table";
    };
    U.$("#huff-run").addEventListener("click", run);
    U.$("#huff-input").addEventListener("keydown", (e) => { if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) run(); });
    run();
  }

  function renderCodesTable(S) {
    const host = U.$("#codes-table");
    if (!S.dec) return;
    const codes = S.dec.events.filter((e) => e.kind === "code" && e.detail);
    const rows = codes.map((e) => [
      e.label,
      e.detail.simple ? `simple ${e.detail.numSymbols}` : `complex h${e.detail.hskip}`,
      { text: U.num((e.symbols || []).length), num: true },
      { text: U.num(e.bit1 - e.bit0), num: true },
      { text: U.num(e.alphabetSize), num: true },
    ]);
    const built = U.table([{ text: "code" }, { text: "form" }, { text: "symbols", num: true },
                           { text: "bits", num: true }, { text: "alphabet", num: true }], rows);
    built.id = "codes-table";
    host.replaceWith(built);
  }

  /* --- 04 header ------------------------------------------------------ */
  function renderHeaderTable(S) {
    const host = U.$("#header-table");
    if (!S.dec) return;
    const mb = S.dec.metablocks[0];
    const end = mb ? mb.headerEndBit || mb.endBit : 0;
    const rows = S.dec.map
      .filter((m) => m.start < end && m.kind !== "literal" && m.kind !== "cmd")
      .slice(0, 220)
      .map((m) => [
        { text: `${m.start}`, num: true },
        { text: String(m.end - m.start), num: true },
        U.el("span", { class: "ch-" + U.channel(m.kind).css, text: m.label }),
        { text: m.value === undefined ? "" : U.num(m.value), num: true },
      ]);
    const built = U.table([{ text: "bit", num: true }, { text: "len", num: true },
                           { text: "field" }, { text: "value", num: true }], rows);
    built.id = "header-table";
    host.replaceWith(built);
  }

  /* --- 05 commands ---------------------------------------------------- */
  function initCommands() {
    const run = () => {
      const insert = Math.max(0, Number(U.$("#cmd-ins").value) | 0);
      const copy = Math.max(2, Number(U.$("#cmd-copy").value) | 0);
      const last = U.$("#cmd-last").value === "1";
      const pick = T.commandSymbol(insert, copy, last) || T.commandSymbol(insert, copy, false);
      const row = T.CMD_LUT[pick.code];
      const rows = [
        ["symbol", `${pick.code} of 704`],
        ["insert code", `${row.insertCode} → base ${row.insertOffset} + ${row.insertExtra} extra bits`],
        ["copy code", `${row.copyCode} → base ${row.copyOffset} + ${row.copyExtra} extra bits`],
        ["distance", row.distanceCode === 0 ? "implicit: reuse the last one" : `coded, context ${row.distanceContext}`],
        ["extra bits", `${row.insertExtra + row.copyExtra} after the symbol`],
      ];
      U.fill(U.$("#cmd-result"), rows.flatMap(([k, v]) => [U.el("dt", { text: k }), U.el("dd", { text: v })]));
    };
    for (const id of ["#cmd-ins", "#cmd-copy", "#cmd-last"]) U.$(id).addEventListener("input", run);
    U.$("#cmd-last").addEventListener("change", run);
    run();

    const lenTable = (host, table, name) => {
      const rows = table.map(([base, extra], i) => {
        const max = base + (extra ? (1 << extra) - 1 : 0);
        return [{ text: String(i), num: true }, { text: U.num(base), num: true },
                { text: String(extra), num: true }, { text: base === max ? U.num(base) : `${U.num(base)}–${U.num(max)}`, num: true }];
      });
      const built = U.table([{ text: "code", num: true }, { text: "base", num: true },
                             { text: "extra", num: true }, { text: name, num: true }], rows);
      built.id = host.id;
      host.replaceWith(built);
    };
    lenTable(U.$("#ins-table"), T.INSERT_LENGTH, "insert length");
    lenTable(U.$("#copy-table"), T.COPY_LENGTH, "copy length");
  }

  function renderCommandTable(S) {
    const host = U.$("#cmd-table");
    if (!S.enc || !S.enc.commands) return;
    const text = BM.latin1(S.input);
    let pos = 0;
    const rows = S.enc.commands.slice(0, 400).map((c, i) => {
      const litStart = pos;
      pos += c.insertLen;
      const produced = c.dictionary ? c.dictionary.produced : (c.kind === "end" ? "" : text.substr(pos, c.copyLen));
      const row = [
        { text: String(i), num: true },
        { text: String(c.symbol), num: true },
        { text: String(c.insertLen), num: true },
        U.el("span", { class: "ch-literal", text: U.showText(text.substr(litStart, Math.min(c.insertLen, 22))) }),
        { text: c.kind === "end" ? "—" : String(c.copyLen), num: true },
        { text: c.kind === "end" ? "—" : c.dictionary ? "dict" : U.num(c.distance), num: true },
        U.el("span", { class: c.dictionary ? "ch-dict" : "ch-command",
          text: c.kind === "end" ? "(tail literals)" : U.showText(String(produced).slice(0, 26)) }),
      ];
      if (c.kind !== "end") pos += c.dictionary ? c.dictionary.produced.length : c.copyLen;
      return row;
    });
    const built = U.table([{ text: "#", num: true }, { text: "sym", num: true }, { text: "ins", num: true },
                           { text: "literals" }, { text: "copy", num: true }, { text: "dist", num: true },
                           { text: "produces" }], rows);
    built.id = "cmd-table";
    host.replaceWith(built);
  }

  /* --- 06 distances --------------------------------------------------- */
  function initDistances() {
    const run = () => {
      const distance = Math.max(1, Number(U.$("#dist-value").value) | 0);
      const npostfix = Number(U.$("#dist-npostfix").value);
      const ndirectRaw = Math.max(0, Math.min(120, Number(U.$("#dist-ndirect").value) | 0));
      const ndirect = (ndirectRaw >> npostfix) << npostfix;
      const pos = Math.max(0, Number(U.$("#dist-pos").value) | 0);
      const coder = BM.encoderInternals.distanceCoder(npostfix, ndirect);
      const cache = [16, 15, 11, 4];
      const hit = coder.encode(distance, cache, 0);
      const windowBits = 22;
      const maxDistance = Math.min(pos, (1 << windowBits) - T.WINDOW_GAP);
      const isDict = distance > maxDistance;
      const rows = [
        ["alphabet", `${coder.alphabetSize} symbols (16 short + ${ndirect} direct + ${24 << (npostfix + 1)} coded)`],
        ["symbol", hit ? String(hit.code) : "not encodable"],
        ["extra bits", hit ? `${hit.extraBits} carrying ${hit.extra}` : "—"],
        ["kind", hit && hit.code < 16
          ? `short code: ${T.DIST_SHORT_NAME[hit.code]} of [${cache.join(", ")}]`
          : hit && hit.code < 16 + ndirect ? "direct distance" : "coded distance"],
        ["at position " + U.num(pos), isDict
          ? `beyond the window: dictionary entry ${U.num(distance - maxDistance - 1)}`
          : `a real copy (largest legal distance here is ${U.num(maxDistance)})`],
      ];
      U.fill(U.$("#dist-result"), rows.flatMap(([k, v]) => [U.el("dt", { text: k }), U.el("dd", { text: v })]));
    };
    for (const id of ["#dist-value", "#dist-npostfix", "#dist-ndirect", "#dist-pos"]) {
      U.$(id).addEventListener("input", run);
      U.$(id).addEventListener("change", run);
    }
    run();
    const cache = [16, 15, 11, 4];
    const rows = T.DIST_SHORT_NAME.map((name, code) => [
      { text: String(code), num: true }, name,
      `cache[${T.DIST_SHORT_INDEX[code]}] ${T.DIST_SHORT_DELTA[code] >= 0 ? "+" : "−"} ${Math.abs(T.DIST_SHORT_DELTA[code])}`,
      { text: U.num(cache[(0 + T.DIST_SHORT_INDEX[code]) & 3] + T.DIST_SHORT_DELTA[code]), num: true },
      code === 0 ? "does not move the ring" : "",
    ]);
    const built = U.table([{ text: "code", num: true }, { text: "means" }, { text: "reads" },
                           { text: "at start", num: true }, { text: "note" }], rows);
    built.id = "dist-short-table";
    U.$("#dist-short-table").replaceWith(built);
  }

  /* --- 07 contexts ---------------------------------------------------- */
  function initContexts() {
    const run = () => {
      const p1 = (U.$("#ctx-p1").value || " ").charCodeAt(0) & 0xff;
      const p2 = (U.$("#ctx-p2").value || " ").charCodeAt(0) & 0xff;
      const table = lut();
      const rows = T.CONTEXT_MODES.map((name, mode) => {
        const ctx = table[(mode << 9) + p1] | table[(mode << 9) + 256 + p2];
        return [name, { text: String(table[(mode << 9) + p1]), num: true },
                { text: String(table[(mode << 9) + 256 + p2]), num: true },
                { text: String(ctx), num: true }];
      });
      const built = U.table([{ text: "mode" }, { text: "from p1", num: true },
                             { text: "from p2", num: true }, { text: "context", num: true }], rows);
      const host = U.$("#ctx-result");
      U.fill(host, [built]);
    };
    U.$("#ctx-p1").addEventListener("input", run);
    U.$("#ctx-p2").addEventListener("input", run);
    run();
  }

  const TREE_HUES = ["#d8a15a", "#6f9bd1", "#79b473", "#c77dbb", "#5fb3b3", "#e08a5a", "#9a8fd8", "#c9a227"];
  function renderContextMap(S) {
    const grid = U.$("#ctx-grid");
    if (!S.enc || !S.enc.clustering) return;
    const cl = S.enc.clustering;
    const hist = S.enc.histograms ? S.enc.histograms.literalByContext : null;
    U.fill(U.$("#ctx-map-note"), [document.createTextNode(
      `${cl.numTrees} tree${cl.numTrees === 1 ? "" : "s"} over 64 contexts, settled in ${cl.iterations} pass${cl.iterations === 1 ? "" : "es"}. Each cell is one context; the number is how many literals landed in it.`)]);
    U.clear(grid);
    for (let c = 0; c < 64; c++) {
      const total = hist ? hist[c].reduce((a, b) => a + b, 0) : 0;
      const cell = U.el("div", {
        class: "cell", "data-empty": total ? "0" : "1",
        title: `context ${c}: ${U.num(total)} literals → tree ${cl.map[c]}`,
        text: total ? (total > 999 ? Math.round(total / 1000) + "k" : String(total)) : "",
      });
      if (total) cell.style.background = TREE_HUES[cl.map[c] % TREE_HUES.length];
      grid.appendChild(cell);
    }
    const trees = cl.histograms.map((h, i) => {
      let total = 0, entropy = 0, symbols = 0;
      for (const v of h) total += v;
      for (const v of h) if (v) { symbols++; entropy -= (v / total) * Math.log2(v / total); }
      const contexts = Array.from(cl.map).filter((t) => t === i).length;
      const swatch = U.el("span", { class: "swatch" });
      swatch.style.background = TREE_HUES[i % TREE_HUES.length];
      return U.el("div", { class: "row" }, [
        U.el("span", { class: "pill" }, [swatch, `tree ${i}`]),
        U.el("span", { class: "note mono",
          text: `${U.num(contexts)} contexts · ${U.num(total)} literals · ${symbols} symbols · ${entropy.toFixed(2)} bits each` }),
      ]);
    });
    U.fill(U.$("#ctx-trees"), trees);
  }

  /* --- 08 dictionary -------------------------------------------------- */
  function initDictionary() {
    const dict = BM.dictionary();
    const search = () => {
      const q = U.$("#dict-query").value;
      const hits = dict.search(q, 80);
      const rows = hits.map((h) => [
        { text: String(h.len), num: true }, { text: String(h.index), num: true },
        U.showText(h.word), h.exact ? "exact" : "",
        { text: U.num(dict.entryId(h.len, h.index, 0)), num: true },
      ]);
      const built = U.table([{ text: "len", num: true }, { text: "index", num: true },
                             { text: "word" }, { text: "" }, { text: "id (identity)", num: true }],
        rows.length ? rows : [["—", "", "no match", "", ""]]);
      built.id = "dict-results";
      U.$("#dict-results").replaceWith(built);
    };
    U.$("#dict-search").addEventListener("click", search);
    U.$("#dict-query").addEventListener("keydown", (e) => { if (e.key === "Enter") search(); });
    search();

    const transform = () => {
      const word = U.$("#tf-word").value;
      const t = Math.max(0, Math.min(120, Number(U.$("#tf-index").value) | 0));
      const parts = dict.transformParts(t);
      const rows = [
        ["transform " + t, dict.describeTransform(t)],
        ["prefix", parts.prefix ? U.showText(parts.prefix) : "(none)"],
        ["operation", parts.typeName],
        ["suffix", parts.suffix ? U.showText(parts.suffix) : "(none)"],
        ["result", U.showText(dict.applyTransform(word, t))],
      ];
      U.fill(U.$("#tf-result"), rows.flatMap(([k, v]) => [U.el("dt", { text: k }), U.el("dd", { text: v })]));
      const all = Array.from({ length: dict.transforms.length }, (_, i) => [
        { text: String(i), num: true },
        dict.describeTransform(i),
        U.showText(dict.applyTransform(word, i)),
      ]);
      const built = U.table([{ text: "#", num: true }, { text: "transform" }, { text: "applied" }], all);
      built.id = "tf-table";
      U.$("#tf-table").replaceWith(built);
    };
    U.$("#tf-word").addEventListener("input", transform);
    U.$("#tf-index").addEventListener("input", transform);
    transform();

    const spell = () => {
      const s = U.$("#dict-spell").value;
      const hits = dict.matchesAt(s, 0, {}).slice(0, 40);
      const rows = hits.map((h) => [
        { text: String(h.matched), num: true },
        U.showText(h.produced),
        `${h.len}·${h.wordIndex} ${U.showText(dict.wordText(h.len, h.wordIndex))}`,
        `${h.transform}: ${dict.describeTransform(h.transform)}`,
        { text: U.num(h.id), num: true },
      ]);
      const built = U.table([{ text: "bytes", num: true }, { text: "produces" },
                             { text: "word" }, { text: "transform" }, { text: "entry id", num: true }],
        rows.length ? rows : [["—", "nothing in the dictionary starts this string", "", "", ""]]);
      built.id = "dict-spell-out";
      U.$("#dict-spell-out").replaceWith(built);
    };
    U.$("#dict-spell-run").addEventListener("click", spell);
    U.$("#dict-spell").addEventListener("keydown", (e) => { if (e.key === "Enter") spell(); });
    spell();

    U.$("#dict-selftest").addEventListener("click", () => {
      const out = U.$("#dict-selftest-out");
      out.textContent = "decoding…";
      setTimeout(() => {
        try {
          const t0 = performance.now();
          const stream = BM.base64ToBytes(BM.data.dictionaryBrBase64);
          const got = BM.decode(stream, { trace: false });
          const want = BM.dictionary().bytes;
          const same = got.output.length === want.length && got.output.every((b, i) => b === want[i]);
          out.className = "note mono " + (same ? "ch-literal" : "ch-command");
          out.textContent = same
            ? `${U.num(stream.length)} bytes in → ${U.num(got.output.length)} bytes out, identical to the embedded dictionary (${(performance.now() - t0).toFixed(0)} ms)`
            : `mismatch: ${U.num(got.output.length)} bytes out`;
        } catch (e) {
          out.className = "note mono ch-command";
          out.textContent = "failed: " + e.message;
        }
      }, 10);
    });
  }

  /* --- 09 encoder stages ---------------------------------------------- */
  function renderEncoderStages(S) {
    if (!S.enc) return;
    const kv = (host, rows) => U.fill(U.$(host), rows.flatMap(([k, v]) =>
      [U.el("dt", { text: k }), U.el("dd", { text: String(v) })]));
    const cmds = S.enc.commands || [];
    const copies = cmds.filter((c) => c.kind === "copy");
    const dicts = cmds.filter((c) => c.kind === "dictionary");
    const literals = cmds.reduce((a, c) => a + c.insertLen, 0);
    const copied = copies.reduce((a, c) => a + c.copyLen, 0);
    const fromDict = dicts.reduce((a, c) => a + c.dictionary.produced.length, 0);
    kv("#enc-stage1", [
      ["commands", U.num(cmds.length)],
      ["literals", `${U.num(literals)} bytes (${U.pct(literals, S.input.length)})`],
      ["copies", `${U.num(copies.length)} covering ${U.num(copied)} bytes (${U.pct(copied, S.input.length)})`],
      ["dictionary", `${U.num(dicts.length)} covering ${U.num(fromDict)} bytes (${U.pct(fromDict, S.input.length)})`],
      ["longest copy", copies.length ? U.num(Math.max(...copies.map((c) => c.copyLen))) : "—"],
      ["mean copy", copies.length ? (copied / copies.length).toFixed(1) : "—"],
      ["reused last distance", U.num(cmds.filter((c) => c.distanceCode === 0 && c.kind !== "end").length)],
    ]);
    const cl = S.enc.clustering;
    const hist = S.enc.histograms;
    let litTotal = 0, litEntropy = 0;
    if (hist) {
      for (const v of hist.literal) litTotal += v;
      for (const v of hist.literal) if (v) litEntropy -= (v / litTotal) * Math.log2(v / litTotal);
    }
    kv("#enc-stage2", [
      ["literal trees", cl ? cl.numTrees : 1],
      ["clustering passes", cl ? cl.iterations : 0],
      ["order-0 literal entropy", `${litEntropy.toFixed(3)} bits/byte`],
      ["context-split entropy", cl ? `${contextEntropy(cl, hist).toFixed(3)} bits/byte` : "—"],
      ["command symbols used", hist ? U.num(hist.command.filter((c) => c > 0).length) : "—"],
      ["distance symbols used", hist ? U.num(hist.distance.filter((c) => c > 0).length) : "—"],
    ]);
    const cls = S.enc.stages.codeLengths;
    const describe = (lengths) => {
      const used = lengths.filter((l) => l > 0).length;
      const max = lengths.reduce((a, l) => Math.max(a, l), 0);
      return `${used} symbols, longest ${max} bits`;
    };
    kv("#enc-stage3", [
      ...(cls.literalLengths.map((l, i) => [`literal code ${i}`, describe(l)])),
      ["insert&copy code", describe(cls.commandLengths)],
      ["distance code", describe(cls.distanceLengths)],
      ["header", `${U.num(S.enc.headerBits || 0)} bits (${U.pct(S.enc.headerBits || 0, S.enc.bits)})`],
    ]);

    /* where the bits went, by channel, counted once each */
    if (S.dec) {
      const owner = U.ownership(S.dec.map, S.enc.bytes.length * 8);
      const totals = new Map();
      for (let b = 0; b < owner.length; b++) {
        const entry = owner[b] >= 0 ? S.dec.map[owner[b]] : null;
        const key = entry ? U.channel(entry.kind).label : "padding";
        totals.set(key, (totals.get(key) || 0) + 1);
      }
      const max = Math.max(...totals.values());
      const bars = Array.from(totals.entries()).sort((a, b) => b[1] - a[1]).map(([label, bits]) => {
        const fill = U.el("span", { class: "fill" });
        fill.style.width = `${(bits / max) * 100}%`;
        const kindKey = Object.entries(U.CHANNELS).find(([, v]) => v.label === label);
        if (kindKey) fill.style.background = U.channelColor(kindKey[0]);
        return U.el("div", { class: "b" }, [
          U.el("span", { text: label }),
          U.el("span", { class: "track" }, [fill]),
          U.el("span", { class: "n", text: `${U.num(Math.round(bits / 8))} B` }),
        ]);
      });
      U.fill(U.$("#enc-stage4"), bars);
    }
  }

  function contextEntropy(cl, hist) {
    if (!hist) return 0;
    let bits = 0, total = 0;
    cl.histograms.forEach((h) => {
      let t = 0;
      for (const v of h) t += v;
      for (const v of h) if (v) bits += v * -Math.log2(v / t);
      total += t;
    });
    return total ? bits / total : 0;
  }

  function initSweep() {
    U.$("#sweep-run").addEventListener("click", () => {
      const S = U.state;
      const host = U.$("#sweep-table");
      const busy = U.table([{ text: "running…" }], []);
      busy.id = "sweep-table";
      host.replaceWith(busy);
      setTimeout(() => {
        const { results, best } = BM.encodeBest(S.input, { plugins: S.overrides });
        const rows = results.map((r) => [
          r.label, r.mode === undefined ? "" : String(r.mode),
          { text: r.error ? "—" : U.num(r.size), num: true },
          { text: r.trees === undefined ? "" : String(r.trees), num: true },
          r.error ? r.error : (best && r.size === best.result.bytes.length ? "smallest" : ""),
        ]);
        const built = U.table([{ text: "literal trees" }, { text: "context mode" },
                               { text: "bytes", num: true }, { text: "trees kept", num: true },
                               { text: "" }], rows);
        built.id = "sweep-table";
        U.$("#sweep-table").replaceWith(built);
      }, 10);
    });
  }

  U.initSections = function () {
    initCodes();
    initCommands();
    initDistances();
    initContexts();
    initDictionary();
    initSweep();
    U.onUpdate((S) => {
      renderCodesTable(S);
      renderHeaderTable(S);
      renderCommandTable(S);
      renderContextMap(S);
      renderEncoderStages(S);
    });
  };
})(globalThis.BM || (globalThis.BM = {}));
