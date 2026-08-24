/* Page helpers: DOM shorthand, the channel palette shared by every view,
   the bit map, and the two pseudocode listings. */
(function (BM) {
  "use strict";
  const U = (BM.ui = BM.ui || {});

  U.$ = (sel, root) => (root || document).querySelector(sel);
  U.$$ = (sel, root) => Array.from((root || document).querySelectorAll(sel));
  U.el = function (tag, attrs, children) {
    const node = document.createElement(tag);
    if (attrs) {
      for (const [k, v] of Object.entries(attrs)) {
        if (v === null || v === undefined || v === false) continue;
        if (k === "class") node.className = v;
        else if (k === "text") node.textContent = v;
        else if (k === "html") node.innerHTML = v;
        else if (k.startsWith("on") && typeof v === "function") node.addEventListener(k.slice(2), v);
        else node.setAttribute(k, v);
      }
    }
    for (const child of [].concat(children || [])) {
      if (child === null || child === undefined || child === false) continue;
      node.appendChild(typeof child === "object" ? child : document.createTextNode(String(child)));
    }
    return node;
  };
  U.clear = (node) => { while (node && node.firstChild) node.removeChild(node.firstChild); return node; };
  U.fill = (node, children) => { U.clear(node); for (const c of [].concat(children)) if (c) node.appendChild(c); return node; };
  U.num = (n) => (typeof n === "number" ? n.toLocaleString("en-US") : n);
  U.pct = (a, b) => (b ? ((a / b) * 100).toFixed(1) + "%" : "—");
  U.bytes = (n) => `${U.num(n)} byte${n === 1 ? "" : "s"}`;

  /* Printable rendering of one byte. */
  U.glyph = function (byte) {
    if (byte === 10) return "\\n";
    if (byte === 13) return "\\r";
    if (byte === 9) return "\\t";
    if (byte < 32 || byte === 127) return "·";
    if (byte > 126) return "·";
    return String.fromCharCode(byte);
  };
  U.escapeText = (s) => Array.from(s).map((ch) => U.glyph(ch.charCodeAt(0))).join("");
  U.quote = (s) => JSON.stringify(s).replace(/^"|"$/g, "");
  /* Printable, quoted, without JSON's second layer of backslashes. */
  U.showText = (s) => "\u201c" + U.escapeText(String(s)) + "\u201d";

  /* One channel per class of field; the colours are the CSS custom properties. */
  U.CHANNELS = {
    stream: { css: "stream", label: "stream header" },
    mb: { css: "header", label: "meta-block header" },
    map: { css: "header", label: "context map" },
    code: { css: "code", label: "prefix codes" },
    block: { css: "block", label: "block switch" },
    cmd: { css: "command", label: "commands" },
    literal: { css: "literal", label: "literals" },
    dist: { css: "distance", label: "distances" },
    copy: { css: "command", label: "copies" },
    dict: { css: "dict", label: "dictionary" },
  };
  U.channel = (kind) => U.CHANNELS[kind] || { css: "block", label: kind };
  U.channelColor = (kind) => `var(--c-${U.channel(kind).css})`;

  /* Per-bit ownership: the shortest map entry covering each bit wins, so a
     summary span never hides the field inside it. */
  U.ownership = function (map, bitLength) {
    const owner = new Int32Array(bitLength).fill(-1);
    const order = map.map((m, i) => i).sort((a, b) => (map[b].end - map[b].start) - (map[a].end - map[a].start));
    for (const i of order) {
      const m = map[i];
      const end = Math.min(m.end, bitLength);
      for (let b = Math.max(0, m.start); b < end; b++) owner[b] = i;
    }
    return owner;
  };

  /* Bytes as hex, tinted by the field that owns most of each byte. */
  U.renderBitmap = function (node, bytes, map, opts) {
    const options = opts || {};
    const limit = Math.min(bytes.length, options.limit || 2048);
    const owner = U.ownership(map, bytes.length * 8);
    U.clear(node);
    const perByte = [];
    for (let i = 0; i < limit; i++) {
      const counts = new Map();
      for (let b = i * 8; b < i * 8 + 8; b++) {
        const o = owner[b];
        counts.set(o, (counts.get(o) || 0) + 1);
      }
      let bestIdx = -1, bestCount = -1;
      for (const [o, c] of counts) if (c > bestCount) { bestCount = c; bestIdx = o; }
      perByte.push(bestIdx);
    }
    const perRow = 16;
    for (let row = 0; row * perRow < limit; row++) {
      const line = U.el("div", { class: "line" });
      line.appendChild(U.el("span", { class: "rowlabel", text: (row * perRow).toString(16).padStart(4, "0") }));
      for (let i = row * perRow; i < Math.min(limit, (row + 1) * perRow); i++) {
        const idx = perByte[i];
        const entry = idx >= 0 ? map[idx] : null;
        const kind = entry ? entry.kind : "mb";
        const span = U.el("span", {
          class: "byte",
          text: bytes[i].toString(16).padStart(2, "0"),
          "data-byte": i,
          title: entry ? `byte ${i}: ${entry.label}` : `byte ${i}`,
        });
        span.style.borderTopColor = U.channelColor(kind);
        span.style.color = entry ? U.channelColor(kind) : "var(--faint)";
        line.appendChild(span);
      }
      node.appendChild(line);
    }
    if (bytes.length > limit) {
      node.appendChild(U.el("div", { class: "note mono", text: `… ${U.num(bytes.length - limit)} more bytes` }));
    }
    return { owner, perByte };
  };

  /* Bit-level breakdown of one byte. */
  U.renderByteFields = function (node, byteIndex, bytes, map) {
    const first = byteIndex * 8, last = first + 8;
    const hits = map
      .map((m, i) => ({ m, i }))
      .filter(({ m }) => m.start < last && m.end > first)
      .sort((a, b) => (a.m.end - a.m.start) - (b.m.end - b.m.start))
      .slice(0, 14)
      .sort((a, b) => a.m.start - b.m.start);
    const value = bytes[byteIndex];
    const strip = U.el("div", { class: "bitstrip" });
    for (let b = 0; b < 8; b++) {
      const bit = (value >> b) & 1;
      const owner = hits.find(({ m }) => m.start <= first + b && m.end > first + b);
      const cell = U.el("span", { class: "bit" + (bit ? "" : " zero"), text: String(bit) });
      cell.style.background = owner ? U.channelColor(owner.m.kind) : "var(--rule)";
      cell.title = owner ? owner.m.label : "";
      strip.appendChild(cell);
    }
    const rows = hits.map(({ m }) => U.el("tr", null, [
      U.el("td", { class: "num", text: `${m.start}–${m.end}` }),
      U.el("td", null, [U.el("span", { class: "ch-" + U.channel(m.kind).css, text: m.label })]),
      U.el("td", { class: "num", text: m.value === undefined ? "" : String(m.value) }),
    ]));
    U.fill(node, [
      U.el("div", { class: "spread" }, [
        U.el("span", { class: "note mono", html: `byte <b>${byteIndex}</b> = 0x${value.toString(16).padStart(2, "0")}` }),
        U.el("span", { class: "note mono", text: "bit 0 first →" }),
      ]),
      strip,
      U.el("table", null, [
        U.el("thead", null, [U.el("tr", null, [
          U.el("th", { class: "num", text: "bits" }), U.el("th", { text: "field" }), U.el("th", { class: "num", text: "value" }),
        ])]),
        U.el("tbody", null, rows),
      ]),
    ]);
  };

  /* --- pseudocode ---------------------------------------------------- */
  U.DECODER_PSEUDO = [
    ["", "read WBITS                                  # 1..7 bits, window = 2^WBITS - 16", "wbits"],
    ["", "", null],
    ["", "repeat                                      # one meta-block per pass", "islast"],
    [1, "ISLAST <- 1 bit", "islast"],
    [1, "if ISLAST and 1 bit ISLASTEMPTY: stop", "islast"],
    [1, "MNIBBLES <- 2 bits; MLEN <- 4*(MNIBBLES+4) bits + 1", "mlen"],
    [1, "if MNIBBLES = 3: skip a metadata block; continue", "mlen"],
    [1, "if not ISLAST and 1 bit ISUNCOMPRESSED:", "uncompressed"],
    [2, "align to byte; copy MLEN bytes; continue", "uncompressed"],
    ["", "", null],
    [1, "for each of literal, insert&copy, distance:   # 9.2", "blocktypes"],
    [2, "NBLTYPES <- 1 + variable-length count", "blocktypes"],
    [2, "if NBLTYPES > 1: read type code, count code,", "blocktypes"],
    [3, "and the first block count", "blocktypes"],
    [1, "NPOSTFIX <- 2 bits; NDIRECT <- 4 bits << NPOSTFIX", "distparams"],
    [1, "for each literal block type: context mode <- 2 bits", "ctxmodes"],
    [1, "literal context map  over NBLTYPESL * 64 slots", "ctxmap"],
    [1, "distance context map over NBLTYPESD * 4  slots", "ctxmap"],
    [1, "read NTREESL literal codes, NBLTYPESI command codes,", "prefixcodes"],
    [2, "NTREESD distance codes                    # 3.5", "prefixcodes"],
    ["", "", null],
    [1, "remaining <- MLEN", "headerdone"],
    [1, "while remaining > 0:", "cmdsym"],
    [2, "if command block count exhausted: switch block", "blockswitch"],
    [2, "sym <- read command symbol                 # 0..703", "cmdsym"],
    [2, "insert <- base[sym] + extra bits", "cmdextra"],
    [2, "copy   <- base[sym] + extra bits", "cmdextra"],
    ["", "", null],
    [2, "repeat insert times:                       # literals", "literal"],
    [3, "if literal block count exhausted: switch block", "blockswitch"],
    [3, "ctx  <- LUT[mode][p1] | LUT[mode][256 + p2]", "literal"],
    [3, "tree <- literal_map[blocktype*64 + ctx]", "literal"],
    [3, "emit byte read with that tree", "literal"],
    [3, "remaining <- remaining - 1", "literal"],
    [2, "if remaining = 0: break                    # copy never happens", "literal"],
    ["", "", null],
    [2, "if sym < 128:", "distance"],
    [3, "distance <- last distance                  # implicit", "distance"],
    [2, "else:", "distance"],
    [3, "dctx <- min(copy - 2, 3)", "distance"],
    [3, "dsym <- read with distance_map[blocktype*4 + dctx]", "distance"],
    [3, "distance <- cache lookup, direct code, or", "distance"],
    [4, "offset + extra bits << NPOSTFIX", "distance"],
    ["", "", null],
    [2, "max <- min(bytes written, 2^WBITS - 16)", "copy"],
    [2, "if distance > max:                          # 8.", "dictref"],
    [3, "id <- distance - max - 1", "dictref"],
    [3, "word <- dictionary[copy][id mod 2^bits[copy]]", "dictref"],
    [3, "emit transform[id >> bits[copy]] applied to word", "dictref"],
    [2, "else:", "copy"],
    [3, "push distance into the cache unless code was 0", "copy"],
    [3, "copy `copy` bytes from `distance` back", "copy"],
    [2, "remaining <- remaining - bytes emitted", "copy"],
    ["", "until ISLAST", "islast"],
  ];

  U.ENCODER_PSEUDO = [
    ["", "# None of this is in the specification. It is all choice.", null],
    ["", "params <- window bits, NPOSTFIX, NDIRECT, context mode,", "params"],
    [3, "how many literal trees to pay for", "params"],
    ["", "", null],
    ["", "# 1. what to say", null],
    ["", "pos <- 0; commands <- []", "match"],
    ["", "while pos < length:", "match"],
    [1, "copy <- longest hash-chain match at pos, scored in bits", "match"],
    [1, "dict <- longest dictionary word (+ transform) at pos", "match"],
    [1, "best <- better of the two", "match"],
    [1, "if a match one byte later beats it: emit a literal", "match"],
    [1, "if best pays for itself:", "match"],
    [2, "commands += (literals since last match, best)", "match"],
    [2, "pos += length produced", "match"],
    [1, "else: pos += 1", "match"],
    ["", "", null],
    ["", "# 2. how often things happen", null],
    ["", "for each command: count its symbol, its distance symbol,", "hist"],
    [3, "and each literal under its context", "hist"],
    ["", "", null],
    ["", "# 3. how many trees to pay for", null],
    ["", "clusters <- group the 64 context histograms (Lloyd,", "cluster"],
    [3, "cross-entropy cost) into k trees", "cluster"],
    ["", "", null],
    ["", "# 4. codes", null],
    ["", "for each alphabet: lengths <- package-merge(counts, 15)", "codes"],
    [1, "write simple form if <= 4 symbols, else the", "codes"],
    [2, "code-length sequence with zero runs", "codes"],
    ["", "", null],
    ["", "# 5. bits", null],
    ["", "write WBITS, ISLAST, MLEN, block counts, NPOSTFIX,", "write"],
    [3, "NDIRECT, context modes, maps, codes", "write"],
    ["", "for each command:", "write"],
    [1, "write its symbol and extra bits", "write"],
    [1, "write each literal with the tree its context selects", "write"],
    [1, "write the distance unless the symbol implied it", "write"],
    [1, "update the distance cache exactly as a decoder would", "write"],
  ];

  U.renderPseudo = function (node, listing, activeId) {
    U.clear(node);
    listing.forEach(([indent, text, id], i) => {
      const line = U.el("div", {
        class: "pl" + (id && id === activeId ? " on" : ""),
        "data-id": id || "",
      }, [
        U.el("span", { class: "ln", text: text ? String(i + 1) : "" }),
        U.el("span", { text: (indent ? "  ".repeat(indent) : "") + text }),
      ]);
      node.appendChild(line);
    });
  };
  U.highlightPseudo = function (node, activeId) {
    let first = null;
    for (const line of node.children) {
      const on = line.dataset.id === activeId && activeId;
      line.classList.toggle("on", !!on);
      if (on && !first) first = line;
    }
    if (first) {
      const top = first.offsetTop - node.clientHeight / 2;
      node.scrollTo({ top: Math.max(0, top), behavior: "smooth" });
    }
  };

  /* Which pseudocode line a trace event belongs to. */
  U.eventLine = function (ev) {
    switch (ev.kind) {
      case "stream": return "wbits";
      case "mb":
        if (/ISLAST/.test(ev.label)) return "islast";
        if (/MNIBBLES|MLEN|meta-block length|metadata|MSKIP|reserved/.test(ev.label)) return "mlen";
        if (/ISUNCOMPRESSED/.test(ev.label)) return "uncompressed";
        if (/NBLTYPES/.test(ev.label)) return "blocktypes";
        if (/NPOSTFIX|NDIRECT/.test(ev.label)) return "distparams";
        if (/context mode/.test(ev.label)) return "ctxmodes";
        if (/NTREES/.test(ev.label)) return "ctxmap";
        if (/header complete/.test(ev.label)) return "headerdone";
        return "mlen";
      case "map": return "ctxmap";
      case "code": return "prefixcodes";
      case "block": return "blockswitch";
      case "cmd": return /extra/.test(ev.label) ? "cmdextra" : "cmdsym";
      case "literal": return "literal";
      case "dist": return "distance";
      case "copy": return "copy";
      case "dict": return "dictref";
      default: return null;
    }
  };

  /* Small table builder. */
  U.table = function (headers, rows, opts) {
    const o = opts || {};
    return U.el("table", null, [
      U.el("thead", null, [U.el("tr", null, headers.map((h) =>
        U.el("th", { class: h.num ? "num" : null, text: h.text !== undefined ? h.text : h })))]),
      U.el("tbody", null, rows.map((r) => {
        const tr = U.el("tr", { class: r.cls || null }, (r.cells || r).map((c) =>
          typeof c === "object" && c !== null && !(c instanceof Node)
            ? U.el("td", { class: c.num ? "num" : c.cls || null }, [c.node || String(c.text)])
            : U.el("td", null, [c instanceof Node ? c : String(c)])));
        if (r.onclick) tr.addEventListener("click", r.onclick);
        return tr;
      })),
    ]);
  };
})(globalThis.BM || (globalThis.BM = {}));
