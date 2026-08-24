/* Section 02: encode, decode, and step through the decode. */
(function (BM) {
  "use strict";
  const U = BM.ui;
  const T = BM.tables;

  const S = (BM.ui.state = {
    input: new Uint8Array(0), enc: null, dec: null, error: null,
    events: [], visible: [], cursor: 0, hidden: new Set(), timer: null,
    sample: null, overrides: {},
  });
  const listeners = [];
  U.onUpdate = (fn) => { listeners.push(fn); return fn; };
  U.notify = () => { for (const fn of listeners) { try { fn(S); } catch (e) { console.error(e); } } };

  U.encodeOptions = function () {
    const trees = U.$("#opt-trees").value;
    return {
      literalTrees: trees === "auto" ? null : Number(trees),
      contextMode: Number(U.$("#opt-mode").value),
      useDictionary: U.$("#opt-dict").value === "1",
      lazy: U.$("#opt-lazy").value === "1",
    };
  };

  /* One press of Encode: run the pipeline, then decode our own bytes. */
  U.run = function () {
    const text = U.$("#input-text").value;
    const input = new TextEncoder().encode(text);
    const opts = U.encodeOptions();
    const plugins = Object.assign({}, BM.defaultPlugins, S.overrides, {
      chooseParams: (ctx) => {
        const base = (S.overrides.chooseParams || BM.defaultPlugins.chooseParams)(ctx);
        return Object.assign(base, {
          literalTrees: opts.literalTrees === null ? base.literalTrees : opts.literalTrees,
          contextMode: opts.contextMode,
          useDictionary: opts.useDictionary, lazy: opts.lazy,
        });
      },
    });
    S.input = input;
    S.error = null;
    const t0 = performance.now();
    try {
      S.enc = BM.encode(input, { plugins });
    } catch (e) {
      S.enc = null; S.dec = null; S.error = { where: "encode", message: e.message, stack: e.stack };
      S.events = []; S.visible = []; S.cursor = 0;
      U.renderAll(); U.notify(); return;
    }
    S.encodeMs = performance.now() - t0;
    const t1 = performance.now();
    try {
      S.dec = BM.decode(S.enc.bytes, { trace: true });
    } catch (e) {
      S.dec = null; S.error = { where: "decode", message: e.message, stack: e.stack };
    }
    S.decodeMs = performance.now() - t1;
    S.events = S.dec ? S.dec.events : [];
    S.cursor = 0;
    U.applyFilter();
    U.renderAll();
    U.notify();
  };

  U.applyFilter = function () {
    S.visible = [];
    for (let i = 0; i < S.events.length; i++) {
      if (!S.hidden.has(S.events[i].kind)) S.visible.push(i);
    }
    if (S.cursor >= S.visible.length) S.cursor = Math.max(0, S.visible.length - 1);
  };

  U.currentEvent = () => (S.visible.length ? S.events[S.visible[S.cursor]] : null);

  /* --- rendering ----------------------------------------------------- */
  U.renderAll = function () {
    renderStats();
    renderBitmapView();
    renderFilters();
    renderTrace();
    renderStep();
  };

  function verdictNode() {
    if (S.error) {
      return U.el("div", { class: "stack" }, [
        U.el("span", { class: "pill bad", text: `${S.error.where} failed` }),
        U.el("div", { class: "console" }, [U.el("span", { class: "err", text: S.error.message })]),
      ]);
    }
    const same = S.dec && S.dec.output.length === S.input.length &&
      S.dec.output.every((b, i) => b === S.input[i]);
    return U.el("div", { class: "row" }, [
      U.el("span", { class: "pill " + (same ? "ok" : "bad"),
        text: same ? "round trip exact" : "round trip differs" }),
      U.el("span", { class: "note mono",
        text: `encode ${S.encodeMs ? S.encodeMs.toFixed(1) : "0"} ms · decode ${S.decodeMs ? S.decodeMs.toFixed(1) : "0"} ms` }),
    ]);
  }

  function renderStats() {
    const node = U.$("#stats");
    if (!S.enc) { U.fill(node, [U.el("div", { class: "note", text: "—" })]); U.fill(U.$("#verdict"), [verdictNode()]); return; }
    const raw = S.input.length;
    const out = S.enc.bytes.length;
    const ref = S.sample && S.sample.brotli11;
    const gz = S.sample && S.sample.gzip9;
    const stat = (label, value, delta, cls) => U.el("div", { class: "stat" }, [
      U.el("div", { class: "value" + (cls ? " " + cls : ""), text: value }),
      U.el("div", { class: "label", text: label }),
      delta ? U.el("div", { class: "delta", text: delta }) : null,
    ]);
    const cmds = S.enc.commands ? S.enc.commands.length : 0;
    const dictRefs = S.enc.commands ? S.enc.commands.filter((c) => c.dictionary).length : 0;
    const copies = S.enc.commands ? S.enc.commands.filter((c) => c.kind === "copy").length : 0;
    U.fill(node, [
      stat("input", U.num(raw), "bytes"),
      stat("this encoder", U.num(out), raw ? (out / raw).toFixed(3) + "× of raw" : "", "brass"),
      stat("brotli q11", ref ? U.num(ref) : "—", ref ? (out <= ref ? `we are ${U.num(ref - out)} smaller` : `we are ${U.num(out - ref)} larger`) : "sample only"),
      stat("gzip -9", gz ? U.num(gz) : "—", gz ? (out / gz).toFixed(2) + "× of gzip" : "sample only"),
      stat("commands", U.num(cmds), `${copies} copies · ${dictRefs} dictionary`),
      stat("header", S.enc.headerBits ? U.num(Math.ceil(S.enc.headerBits / 8)) : "—",
        S.enc.headerBits ? `${U.pct(S.enc.headerBits, S.enc.bits)} of the stream` : ""),
    ]);
    U.fill(U.$("#verdict"), [verdictNode()]);
  }

  function renderBitmapView() {
    const node = U.$("#bitmap");
    if (!S.enc || !S.dec) { U.fill(node, [U.el("div", { class: "note", text: "—" })]); return; }
    U.renderBitmap(node, S.enc.bytes, S.dec.map, { limit: 4096 });
    const kinds = new Set(S.dec.map.map((m) => m.kind));
    U.fill(U.$("#bitmap-legend"), Array.from(kinds).map((k) => {
      const ch = U.channel(k);
      const i = U.el("i");
      i.style.background = U.channelColor(k);
      return U.el("span", null, [i, ch.label]);
    }));
    node.onclick = (e) => {
      const target = e.target.closest("[data-byte]");
      if (!target) return;
      U.$$(".byte.sel", node).forEach((n) => n.classList.remove("sel"));
      target.classList.add("sel");
      U.renderByteFields(U.$("#bitfield"), Number(target.dataset.byte), S.enc.bytes, S.dec.map);
    };
    node.onmousemove = (e) => {
      const target = e.target.closest("[data-byte]");
      if (target) U.$("#bitmap-hint").textContent = target.title;
    };
  }

  function renderFilters() {
    const node = U.$("#tr-filter");
    const kinds = [];
    for (const ev of S.events) if (!kinds.includes(ev.kind)) kinds.push(ev.kind);
    U.fill(node, kinds.map((k) => {
      const on = !S.hidden.has(k);
      const btn = U.el("button", {
        class: "icon", text: U.channel(k).label,
        onclick: () => {
          if (S.hidden.has(k)) S.hidden.delete(k); else S.hidden.add(k);
          U.applyFilter(); renderFilters(); renderTrace(); renderStep();
        },
      });
      btn.style.borderColor = on ? U.channelColor(k) : "var(--rule)";
      btn.style.color = on ? U.channelColor(k) : "var(--faint)";
      btn.style.fontSize = "10.5px";
      btn.style.padding = "6px 8px";
      return btn;
    }));
    U.$("#trace-count").textContent = S.dec
      ? `${U.num(S.visible.length)} of ${U.num(S.events.length)} steps${S.dec.truncated ? " (truncated)" : ""}`
      : "";
    const scrub = U.$("#tr-scrub");
    scrub.max = String(Math.max(0, S.visible.length - 1));
    scrub.value = String(S.cursor);
  }

  const WINDOW = 240;
  function renderTrace() {
    const node = U.$("#trace");
    if (!S.visible.length) { U.fill(node, [U.el("div", { class: "note", text: "no steps" })]); return; }
    const start = Math.max(0, S.cursor - Math.floor(WINDOW / 3));
    const end = Math.min(S.visible.length, start + WINDOW);
    const rows = [];
    for (let i = start; i < end; i++) {
      const ev = S.events[S.visible[i]];
      const ch = U.channel(ev.kind);
      const row = U.el("div", {
        class: "trace-row" + (i === S.cursor ? " is-current" : ""),
        onclick: () => { S.cursor = i; renderTrace(); renderStep(); },
      }, [
        U.el("span", { class: "at", text: String(ev.bit0 !== undefined ? ev.bit0 : ev.bit) }),
        U.el("span", { class: "what ch-" + ch.css, text: ev.label.slice(0, 22) }),
        U.el("span", { class: "detail", text: describe(ev) }),
      ]);
      rows.push(row);
    }
    U.fill(node, rows);
    const current = node.children[S.cursor - start];
    if (current) {
      const top = current.offsetTop - node.clientHeight / 2;
      node.scrollTo({ top: Math.max(0, top) });
    }
    U.$("#tr-scrub").value = String(S.cursor);
  }

  function describe(ev) {
    if (ev.kind === "literal") return `${U.showText(U.glyph(ev.value))} ctx ${ev.context} tree ${ev.treeIndex} · ${ev.bits} bits`;
    if (ev.kind === "copy") return `${ev.copyLen} bytes from ${ev.distance} back → ${U.showText(ev.text || "")}`;
    if (ev.kind === "dict") return `#${ev.value} = ${U.showText(ev.word)} ${ev.transform} → ${U.showText(ev.produced)}`;
    if (ev.kind === "cmd" && ev.insertLen !== undefined) return ev.note;
    if (ev.note) return ev.note;
    return ev.value === undefined ? "" : String(ev.value);
  }

  /* The cache is a ring: recency order is idx-1, idx-2, idx-3, idx-4. */
  function cacheAt(index) {
    let cache = [16, 15, 11, 4], idx = 0;
    for (let i = 0; i <= index && i < S.events.length; i++) {
      if (S.events[i].cache) { cache = S.events[i].cache; idx = S.events[i].cacheIdx; }
    }
    return [3, 2, 1, 0].map((k) => cache[(idx + k) & 3]);
  }

  function renderStep() {
    const ev = U.currentEvent();
    const pseudo = U.$("#pseudo-live");
    if (!ev) {
      U.fill(U.$("#stepdetail"), []);
      U.fill(U.$("#outview"), []);
      U.highlightPseudo(pseudo, null);
      return;
    }
    U.highlightPseudo(pseudo, U.eventLine(ev));

    /* output so far, with what this step produced picked out */
    const idx = S.visible[S.cursor];
    const prev = idx > 0 ? S.events[idx - 1].out : 0;
    const now = ev.out !== undefined ? ev.out : prev;
    const produced = Math.max(0, now - prev);
    const all = S.dec.output;
    const tailStart = Math.max(0, now - 400);
    const before = BM.latin1(all.subarray(tailStart, Math.max(tailStart, now - produced)));
    const fresh = BM.latin1(all.subarray(Math.max(tailStart, now - produced), now));
    const cls = ev.kind === "dict" ? "fromdict" : ev.kind === "copy" ? "fromcopy" : "new";
    const outview = U.$("#outview");
    outview.classList.toggle("empty", now === 0);
    U.fill(outview, now === 0 ? [U.el("span", { text: "nothing decoded yet — this step is still header" })] : [
      tailStart > 0 ? U.el("span", { class: "dim", text: "…" }) : null,
      U.el("span", { class: "old", text: U.escapeText(before) }),
      fresh ? U.el("span", { class: cls, text: U.escapeText(fresh) }) : null,
    ]);

    /* the cache, with the slot the ring points at marked */
    const cache = cacheAt(idx);
    U.fill(U.$("#cache"), cache.map((d, i) => U.el("div", { class: "slot" + (i === 0 ? " hot" : "") }, [
      U.el("div", { class: "n", text: U.num(d) }),
      U.el("div", { class: "t", text: ["last", "2nd", "3rd", "4th"][i] }),
    ])));

    /* everything the event carries */
    const rows = [
      ["step", `${S.cursor + 1} of ${S.visible.length}`],
      ["field", ev.label],
      ["bits", ev.bit0 !== undefined ? `${ev.bit0}–${ev.bit1} (${ev.bit1 - ev.bit0})` : String(ev.bit)],
      ["byte", ev.bit0 !== undefined ? String(ev.bit0 >> 3) : ""],
      ["value", ev.value === undefined ? "" : String(ev.value)],
      ["output", `${U.num(now)} bytes${produced ? ` (+${produced})` : ""}`],
    ];
    if (ev.note) rows.push(["note", ev.note]);
    if (ev.kind === "literal") {
      rows.push(["byte out", `${U.showText(U.glyph(ev.value))} = 0x${ev.value.toString(16).padStart(2, "0")}`]);
      rows.push(["context", `${ev.context} from p1=${U.showText(U.glyph(ev.p1))} p2=${U.showText(U.glyph(ev.p2))} (${ev.mode})`]);
      rows.push(["tree", String(ev.treeIndex)]);
    }
    if (ev.kind === "dict") {
      rows.push(["word", U.showText(ev.word)]);
      rows.push(["transform", ev.transform]);
      rows.push(["produced", U.showText(ev.produced)]);
    }
    if (ev.kind === "code" && ev.detail) {
      rows.push(["form", ev.detail.simple ? `simple, ${ev.detail.numSymbols} symbols` : `complex, HSKIP ${ev.detail.hskip}`]);
      rows.push(["symbols", String((ev.symbols || []).length)]);
    }
    U.fill(U.$("#stepdetail"), rows.flatMap(([k, v]) =>
      [U.el("dt", { text: k }), U.el("dd", { text: String(v) })]));

    /* mark the bytes this field lives in */
    const map = U.$("#bitmap");
    U.$$(".byte.sel", map).forEach((n) => n.classList.remove("sel"));
    if (ev.bit0 !== undefined) {
      for (let b = ev.bit0 >> 3; b <= (ev.bit1 - 1) >> 3; b++) {
        const node = map.querySelector(`[data-byte="${b}"]`);
        if (node) node.classList.add("sel");
      }
      const firstByte = map.querySelector(`[data-byte="${ev.bit0 >> 3}"]`);
      if (firstByte && firstByte.parentElement) {
        const top = firstByte.parentElement.offsetTop - map.clientHeight / 2;
        map.scrollTo({ top: Math.max(0, top) });
      }
    }
  }

  /* --- transport ----------------------------------------------------- */
  function step(delta) {
    if (!S.visible.length) return;
    S.cursor = Math.min(S.visible.length - 1, Math.max(0, S.cursor + delta));
    renderTrace(); renderStep();
  }
  function play() {
    if (S.timer) { stop(); return; }
    const speed = Number(U.$("#tr-speed").value);
    U.$("#tr-play").textContent = "❙❙ pause";
    S.timer = setInterval(() => {
      if (S.cursor >= S.visible.length - 1) { stop(); return; }
      step(1);
    }, speed);
  }
  function stop() {
    if (S.timer) clearInterval(S.timer);
    S.timer = null;
    U.$("#tr-play").textContent = "▶ play";
  }

  U.initMachine = function () {
    const pick = U.$("#sample-pick");
    U.fill(pick, BM.samples.map((s, i) => U.el("option", { value: String(i), text: s.name })));
    const load = (i) => {
      S.sample = BM.samples[i];
      U.$("#input-text").value = S.sample.text;
      U.run();
    };
    pick.addEventListener("change", () => load(Number(pick.value)));
    U.$("#input-text").addEventListener("input", () => { S.sample = null; });
    U.$("#btn-encode").addEventListener("click", () => U.run());
    for (const id of ["#opt-trees", "#opt-mode", "#opt-dict", "#opt-lazy"]) {
      U.$(id).addEventListener("change", () => U.run());
    }
    U.$("#tr-next").addEventListener("click", () => { stop(); step(1); });
    U.$("#tr-back").addEventListener("click", () => { stop(); step(-1); });
    U.$("#tr-first").addEventListener("click", () => { stop(); S.cursor = 0; renderTrace(); renderStep(); });
    U.$("#tr-last").addEventListener("click", () => { stop(); S.cursor = Math.max(0, S.visible.length - 1); renderTrace(); renderStep(); });
    U.$("#tr-play").addEventListener("click", play);
    U.$("#tr-speed").addEventListener("change", () => { if (S.timer) { stop(); play(); } });
    U.$("#tr-scrub").addEventListener("input", (e) => { stop(); S.cursor = Number(e.target.value); renderTrace(); renderStep(); });
    document.addEventListener("keydown", (e) => {
      if (/^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement.tagName)) return;
      if (e.key === "ArrowRight") { stop(); step(e.shiftKey ? 25 : 1); e.preventDefault(); }
      else if (e.key === "ArrowLeft") { stop(); step(e.shiftKey ? -25 : -1); e.preventDefault(); }
      else if (e.key === " ") { play(); e.preventDefault(); }
    });
    U.renderPseudo(U.$("#pseudo-live"), U.DECODER_PSEUDO, null);
    load(0);
  };
})(globalThis.BM || (globalThis.BM = {}));
