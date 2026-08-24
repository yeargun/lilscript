/* Sections 10–12: the listings, the editable stages, the scratchpad. */
(function (BM) {
  "use strict";
  const U = BM.ui;

  /* --- 10 pseudocode + real source ------------------------------------ */
  const SOURCES = () => [
    ["decoder — whole pass", BM.decode],
    ["decoder — read one prefix code", extract(BM.decode, "function readPrefixCode")],
    ["decoder — read a context map", extract(BM.decode, "function readContextMap")],
    ["prefix codes — decode a symbol", BM.huffman.readSymbol],
    ["prefix codes — build a decode table", BM.huffman.buildDecodeTable],
    ["prefix codes — package-merge", BM.huffman.packageMerge],
    ["encoder — whole pass", BM.encode],
    ["encoder — write one prefix code", BM.encoderInternals.writePrefixCode],
    ["encoder — zero-run chaining", BM.encoderInternals.zeroRunCode],
    ["encoder — distance coding", BM.encoderInternals.distanceCoder],
    ["encoder — histograms", BM.encoderInternals.histograms],
    ["dictionary — apply a transform", BM.Dictionary.prototype.applyTransform],
    ["dictionary — find matches", BM.Dictionary.prototype.matchesAt],
    ["bit reader", BM.BitReader],
    ["bit writer", BM.BitWriter],
  ];

  /* Pull a named inner function out of a larger one, for readability. */
  function extract(fn, needle) {
    const src = fn.toString();
    const at = src.indexOf(needle);
    if (at < 0) return src;
    let depth = 0, i = src.indexOf("{", at);
    const start = at;
    for (; i < src.length; i++) {
      if (src[i] === "{") depth++;
      else if (src[i] === "}") { depth--; if (depth === 0) { i++; break; } }
    }
    return dedent(src.slice(start, i));
  }
  function dedent(src) {
    const lines = src.split("\n");
    const indents = lines.slice(1).filter((l) => l.trim()).map((l) => l.match(/^\s*/)[0].length);
    const min = indents.length ? Math.min(...indents) : 0;
    return lines.map((l, i) => (i === 0 ? l : l.slice(min))).join("\n");
  }

  const KEYWORDS = /\b(const|let|var|function|return|if|else|for|while|of|in|new|class|throw|break|continue|switch|case|default|do|typeof|instanceof|this|null|undefined|true|false)\b/;
  function highlight(src) {
    const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    const re = /(\/\*[\s\S]*?\*\/|\/\/[^\n]*)|("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`(?:[^`\\]|\\.)*`)/g;
    let out = "", last = 0, m;
    const words = (text) => esc(text).replace(new RegExp(KEYWORDS.source, "g"), '<span class="kw">$1</span>');
    while ((m = re.exec(src))) {
      out += words(src.slice(last, m.index));
      out += m[1] ? `<span class="cm">${esc(m[1])}</span>` : `<span class="st">${esc(m[2])}</span>`;
      last = m.index + m[0].length;
    }
    out += words(src.slice(last));
    return out;
  }

  function initPseudocode() {
    const tabs = U.$("#pseudo-tabs");
    const listings = [["Decoder", U.DECODER_PSEUDO], ["Encoder", U.ENCODER_PSEUDO]];
    const show = (i) => {
      U.$$("button", tabs).forEach((b, j) => b.classList.toggle("on", i === j));
      U.$("#pseudo-static-title").textContent = listings[i][0];
      U.renderPseudo(U.$("#pseudo-static"), listings[i][1], null);
    };
    U.fill(tabs, listings.map(([label], i) => U.el("button", { text: label, onclick: () => show(i) })));
    show(0);

    const pick = U.$("#src-pick");
    const sources = SOURCES();
    U.fill(pick, sources.map(([label], i) => U.el("option", { value: String(i), text: label })));
    const render = () => {
      const entry = sources[Number(pick.value)][1];
      const src = typeof entry === "string" ? entry : entry.toString();
      U.$("#src-view").innerHTML = highlight(src);
    };
    pick.addEventListener("change", render);
    render();
  }

  /* --- 11 plugin slots ------------------------------------------------ */
  const SLOTS = [
    ["chooseParams", "Window size, NPOSTFIX/NDIRECT, context mode, how many literal trees to pay for, and the match-finder's budget. Returns an object; the page's own controls are applied on top of whatever you return."],
    ["findMatch", "The back-reference search. Given a position, return {kind:'copy', len, distance, score} or null. `score` is in estimated bits saved and is what the caller compares against the dictionary probe."],
    ["dictProbe", "The static-dictionary search. Return {kind:'dictionary', len, wordIndex, transform, produced, distance, score} or null. `len` is the dictionary word's length, which is what the command's copy field carries; `produced` is what the decoder will actually emit."],
    ["buildCommands", "The whole parse: walk the input, call the two searches, decide literal-versus-match, and emit commands. This is where lazy matching and the last-distance strategy live. Everything downstream trusts the commands you return."],
    ["clusterContexts", "Group the 64 literal contexts into trees. Return {map: Uint8Array(64), numTrees, histograms: [Int32Array(256)...]}. Fewer trees cost less header and code literals worse."],
    ["codeLengths", "Counts to code lengths. Must produce a decodable code: lengths ≤ 15, and either complete or a single symbol."],
    ["buildMatchIndex", "The hash index behind findMatch. Return {head, prev, hashAt, insert}."],
  ];
  let activeSlot = 0;

  function compile(name, src) {
    const H = BM.pluginHelpers;
    const factory = new Function(
      "BM", "T", "matchCost", "logModel", "crossEntropy", "dictSuffixTransforms", "upperFirstTransforms",
      `"use strict"; return (${src});`);
    const fn = factory(BM, BM.tables, H.matchCost, H.logModel, H.crossEntropy,
      H.dictSuffixTransforms, H.upperFirstTransforms);
    if (typeof fn !== "function") throw new Error(`${name} must evaluate to a function`);
    return fn;
  }

  function initPlugins() {
    const S = U.state;
    const tabs = U.$("#plugin-tabs");
    const srcBox = U.$("#plugin-src");
    const status = U.$("#plugin-status");
    const edited = {};

    const show = (i) => {
      activeSlot = i;
      U.$$("button", tabs).forEach((b, j) => b.classList.toggle("on", i === j));
      const [name, doc] = SLOTS[i];
      U.$("#plugin-doc").innerHTML = `<code>${name}(…)</code> — ${doc}`;
      srcBox.value = edited[name] !== undefined ? edited[name] : BM.defaultPlugins[name].toString();
      status.className = "status";
      status.textContent = S.overrides[name] ? "this slot is running your version" : "running the default";
    };
    U.fill(tabs, SLOTS.map(([name], i) => U.el("button", { text: name, onclick: () => show(i) })));

    const report = (ok, message) => {
      status.className = "status " + (ok ? "ok" : "bad");
      status.textContent = message;
    };

    U.$("#plugin-run").addEventListener("click", () => {
      const [name] = SLOTS[activeSlot];
      edited[name] = srcBox.value;
      let fn;
      try {
        fn = compile(name, srcBox.value);
      } catch (e) {
        report(false, "did not compile: " + e.message);
        return;
      }
      const before = S.enc ? S.enc.bytes.length : 0;
      const previous = S.overrides[name];
      S.overrides[name] = fn;
      U.run();
      if (S.error) {
        report(false, `${S.error.where} threw: ${S.error.message}`);
      } else {
        const after = S.enc.bytes.length;
        const delta = after - before;
        report(true, `ran: ${U.num(after)} bytes${before ? ` (${delta === 0 ? "no change" : delta > 0 ? "+" + delta : delta} vs previous)` : ""}`);
      }
      if (S.error && S.error.where === "encode") S.overrides[name] = previous;
      renderPluginResult();
    });
    U.$("#plugin-reset").addEventListener("click", () => {
      const [name] = SLOTS[activeSlot];
      delete edited[name];
      delete S.overrides[name];
      show(activeSlot);
      U.run();
      renderPluginResult();
    });
    U.$("#plugin-reset-all").addEventListener("click", () => {
      for (const [name] of SLOTS) { delete edited[name]; delete S.overrides[name]; }
      show(activeSlot);
      U.run();
      renderPluginResult();
    });
    srcBox.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); U.$("#plugin-run").click(); }
    });
    show(0);

    function renderPluginResult() {
      const rows = [];
      const S2 = U.state;
      if (S2.error) {
        rows.push(["status", `${S2.error.where} failed`]);
        rows.push(["message", S2.error.message]);
      } else if (S2.enc) {
        const same = S2.dec && S2.dec.output.length === S2.input.length &&
          S2.dec.output.every((b, i) => b === S2.input[i]);
        rows.push(["round trip", same ? "exact" : "DIFFERS — the stream is not legal"]);
        rows.push(["bytes", `${U.num(S2.input.length)} → ${U.num(S2.enc.bytes.length)}`]);
        rows.push(["commands", U.num(S2.enc.commands.length)]);
        rows.push(["literal trees", S2.enc.clustering ? S2.enc.clustering.numTrees : 1]);
        rows.push(["header", `${U.num(S2.enc.headerBits || 0)} bits`]);
        rows.push(["slots overridden", Object.keys(S2.overrides).join(", ") || "none"]);
      }
      U.fill(U.$("#plugin-result"), rows.flatMap(([k, v]) =>
        [U.el("dt", { text: k }), U.el("dd", { text: String(v) })]));
    }
    U.onUpdate(renderPluginResult);
    renderPluginResult();

    /* scratchpad */
    const scratch = U.$("#scratch-src");
    scratch.value = [
      "// The engine is on BM. This runs on the current input.",
      "const input = BM.ui.state.input;",
      "const r = BM.encode(input);",
      "log('bytes', r.bytes.length, 'commands', r.commands.length);",
      "for (const c of r.commands.slice(0, 5)) log(c.kind, 'insert', c.insertLen, 'copy', c.copyLen, c.dictionary ? JSON.stringify(c.dictionary.produced) : '');",
      "return BM.dictionary().search('script', 5).map(h => h.word);",
    ].join("\n");
    const runScratch = () => {
      const out = U.$("#scratch-out");
      const lines = [];
      const log = (...args) => lines.push(args.map(fmt).join(" "));
      try {
        const fn = new Function("BM", "log", "print", `"use strict";\n${scratch.value}`);
        const value = fn(BM, log, log);
        if (value !== undefined) lines.push("→ " + fmt(value));
        U.fill(out, [document.createTextNode(lines.join("\n") || "(no output)")]);
      } catch (e) {
        U.fill(out, [document.createTextNode(lines.join("\n") + (lines.length ? "\n" : "")),
                     U.el("span", { class: "err", text: e.stack || e.message })]);
      }
    };
    U.$("#scratch-run").addEventListener("click", runScratch);
    scratch.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); runScratch(); }
    });
    let scratchPrimed = false;
    U.onUpdate(() => { if (!scratchPrimed) { scratchPrimed = true; runScratch(); } });
  }

  function fmt(value) {
    if (typeof value === "string") return value;
    if (value instanceof Uint8Array) return `Uint8Array(${value.length}) ${Array.from(value.subarray(0, 24)).map((b) => b.toString(16).padStart(2, "0")).join(" ")}${value.length > 24 ? " …" : ""}`;
    try { return JSON.stringify(value, (k, v) => (v instanceof Int32Array || v instanceof Uint8Array ? Array.from(v.slice(0, 32)) : v), 1); }
    catch (e) { return String(value); }
  }

  /* --- 12 provenance --------------------------------------------------- */
  function initProvenance() {
    const b = BM.build || {};
    const rows = [
      ["built", b.date || "—"],
      ["brotli tables", BM.data.brotliVersion],
      ["dictionary", `${U.num(BM.dictionary().bytes.length)} bytes, sha256 ${BM.data.dictionarySha256.slice(0, 16)}…`],
      ["transforms", `${BM.data.transforms.length}`],
      ["command table", "704 rows, derived and checked against the C decoder"],
      ["build tests", b.tests ? `${b.tests.pass} passed, ${b.tests.fail} failed` : "—"],
      ["decoder", b.tests ? `${b.tests.streams} streams from the real library` : "—"],
      ["encoder", b.tests ? `${b.tests.encoder} round trips + ${b.tests.fuzz} fuzz cases, all read back by the real library` : "—"],
      ["code in this page", b.engineBytes ? `${U.num(b.engineBytes)} bytes over ${b.modules} modules` : "—"],
      ["embedded tables", b.tableBytes ? `${U.num(b.tableBytes)} bytes (dictionary, transforms, context table)` : "—"],
    ];
    U.fill(U.$("#provenance-facts"), rows.flatMap(([k, v]) =>
      [U.el("dt", { text: k }), U.el("dd", { text: String(v) })]));
    if (b.tests) {
      U.$("#fact-tests").textContent = `${b.tests.pass} build checks against the real library`;
    }
    U.$("#fact-version").textContent = BM.data.brotliVersion;
  }

  U.initRest = function () {
    initPseudocode();
    initPlugins();
    initProvenance();
  };
})(globalThis.BM || (globalThis.BM = {}));
