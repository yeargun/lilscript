const data = await fetch("./results.json").then((response) => {
  if (!response.ok) throw new Error(`Unable to load results: ${response.status}`)
  return response.json()
})

const lilMod = await import(`./${data.file}.js`)
const officialMod = await import("./official.js").catch(() => null)

const formatter = new Intl.NumberFormat("en-US")

function pick(mod, name) {
  if (!mod) return null
  if (name && mod[name]) return mod[name]
  return mod.default ?? Object.values(mod)[0] ?? null
}

const lilApi = pick(lilMod, data.lilExport)
const officialApi = pick(officialMod, data.officialExport)

function percentAgainst(value, baseline, better, worse) {
  if (!baseline || value === baseline) {
    return { text: "baseline", amount: "—", word: "baseline", state: "even" }
  }
  const change = (baseline - value) / baseline
  const magnitude = Math.abs(change * 100)
  const digits = magnitude < 10 ? 1 : 0
  const word = change > 0 ? better : worse
  return {
    text: `${magnitude.toFixed(digits)}% ${word}`,
    amount: `${magnitude.toFixed(digits)}%`,
    word,
    state: change > 0 ? "win" : "loss",
  }
}

function smallerThan(value, baseline) {
  return percentAgainst(value, baseline, "smaller", "larger")
}

function fasterThan(value, baseline) {
  return percentAgainst(value, baseline, "faster", "slower")
}

function laneById(id) {
  return data.size.find((lane) => lane.id === id)
}

function barClass(id) {
  if (id === "itslil") return "bar-lil"
  if (id === "itslil-closed") return "bar-closed"
  return "bar-official"
}

function ms(value) {
  return `${value.toFixed(2)} ms`
}

function renderCodec(metric, ids, barId, bodyId) {
  const baseline = data.size.find((lane) => lane.baseline)
  const lanes = ids.map(laneById).filter(Boolean)
  if (!baseline || lanes.length === 0) return
  const max = Math.max(...lanes.map((lane) => lane[metric]))
  document.querySelector(barId).innerHTML = lanes
    .map((lane) => {
      const width = Math.max(18, (lane[metric] / max) * 100)
      return `<div class="${barClass(lane.id)}" style="width:${width}%"><span>${lane.name}</span><strong>${formatter.format(lane[metric])} B</strong></div>`
    })
    .join("")
  document.querySelector(bodyId).innerHTML = lanes
    .map((lane) => {
      const verdict = smallerThan(lane[metric], baseline[metric])
      return `<tr><th scope="row">${lane.name}</th><td>${formatter.format(lane[metric])}</td><td class="verdict ${verdict.state}"><strong>${verdict.text}</strong></td></tr>`
    })
    .join("")
}

function renderHero() {
  const baseline = data.size.find((lane) => lane.baseline)
  const itslil = laneById("itslil")
  if (!baseline || !itslil) return
  const smaller = smallerThan(itslil.brotli11, baseline.brotli11)
  document.querySelector("#hero-ratio").innerHTML = `${smaller.amount}<span>${smaller.word}</span>`
  document.querySelector("#hero-bytes").textContent =
    `${formatter.format(baseline.brotli11)} B → ${formatter.format(itslil.brotli11)} B Brotli-11`
  document.querySelector("#hero-shipped").textContent = smaller.text
  document.querySelector("#hero-gzip").textContent = smallerThan(itslil.gzip9, baseline.gzip9).text
  document.querySelector("#hero-raw").textContent = smallerThan(itslil.raw, baseline.raw).text
  document.querySelector("#hero-spec").textContent = data.spec
    ? `${data.spec.pass}/${data.spec.total}`
    : "—"
}

function renderSize() {
  const baseline = data.size.find((lane) => lane.baseline)
  if (!baseline) return
  const officialIds = data.size.filter((lane) => lane.id.startsWith("official")).map((lane) => lane.id)
  renderCodec("brotli11", [...officialIds, "itslil", "itslil-closed"], "#bar-brotli", "#body-brotli")
  renderCodec("gzip9", [...officialIds, "itslil", "itslil-closed"], "#bar-gzip", "#body-gzip")
  renderCodec("raw", [...officialIds, "itslil", "itslil-closed"], "#bar-raw", "#body-raw")
  document.querySelector("#body-matched").innerHTML = data.size
    .map((lane) => {
      const verdict = smallerThan(lane.brotli11, baseline.brotli11)
      return `<tr><th scope="row">${lane.name}</th><td>${formatter.format(lane.raw)}</td><td>${formatter.format(lane.gzip9)}</td><td>${formatter.format(lane.brotli11)}</td><td class="verdict ${verdict.state}"><strong>${verdict.text}</strong></td></tr>`
    })
    .join("")
}

function renderPerf() {
  const suites = data.throughput ?? []
  const lil = suites.find((row) => row.id === "itslil")
  const official = suites.find((row) => row.id === "official")
  const speed = lil && official ? fasterThan(lil.documentMs, official.documentMs) : null
  const cards = [
    {
      label: data.perfLead ?? "same work, against the official runtime graph",
      value: speed ? speed.text : "—",
      win: speed?.state === "win",
    },
    {
      label: "LilScript median",
      value: lil ? ms(lil.documentMs) : "—",
    },
    {
      label: "official median",
      value: official ? ms(official.documentMs) : "—",
    },
    {
      label: data.spec?.label ?? "tests passing",
      value: data.spec ? `${data.spec.pass}/${data.spec.total}` : "—",
      geo: true,
    },
  ]
  document.querySelector("#perf-cards").innerHTML = cards
    .map(
      (card) =>
        `<article class="perf-card${card.win ? " win" : ""}${card.geo ? " geo" : ""}"><strong>${card.value}</strong><span>${card.label}</span></article>`,
    )
    .join("")
  document.querySelector("#perf-body").innerHTML = suites
    .map((row) => {
      const verdict = official ? fasterThan(row.documentMs, official.documentMs) : null
      return `<tr><th scope="row">${row.name}</th><td>${ms(row.documentMs)}</td><td class="verdict ${verdict ? verdict.state : ""}"><strong>${verdict ? verdict.text : "—"}</strong></td></tr>`
    })
    .join("")
  document.querySelector("#perf-note").textContent =
    `${data.runtime ?? "Node"}. ${data.codec}. Quiet median after discarding the first ${data.warmupDiscard ?? 3} samples.`
}

function bindCopy() {
  document.addEventListener("click", async (event) => {
    const button = event.target.closest("[data-copy]")
    if (!button) return
    await navigator.clipboard.writeText(button.dataset.copy)
    button.textContent = "copied"
    window.setTimeout(() => {
      button.textContent = "copy"
    }, 1200)
  })
}

function bindProgress() {
  const bar = document.querySelector(".progress")
  const update = () => {
    const max = document.documentElement.scrollHeight - window.innerHeight
    bar.style.transform = `scaleX(${max > 0 ? window.scrollY / max : 0})`
  }
  window.addEventListener("scroll", update, { passive: true })
  update()
}

function writeFrame(html) {
  const frame = document.querySelector("#preview")
  const doc = frame.contentDocument
  doc.open()
  doc.write(`<!doctype html><html><head><link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.22/dist/katex.min.css"><style>
    body { margin: 0; padding: 22px 24px; font: 16px/1.55 Manrope, system-ui, sans-serif; color: #1c1814; }
    a { color: #8a5a12; }
    pre { overflow: auto; padding: 12px; background: #f4efe4; }
    code { font-family: "DM Mono", ui-monospace, monospace; font-size: 13px; }
    table { border-collapse: collapse; }
    th, td { border: 1px solid #e0d4c0; padding: 6px 10px; }
    blockquote { border-left: 3px solid #e3b341; padding-left: 12px; color: #6b5e4a; }
  </style></head><body>${html}</body></html>`)
  doc.close()
}

function parseHost(plugin, options = {}) {
  const settings = {}
  const proc = {
    data() {
      return { settings }
    },
  }
  plugin.call(proc, options)
  return proc
}

function currentEngine() {
  return document.querySelector("input[name=engine]:checked")?.value === "official" ? "official" : "lil"
}

function runLil(src) {
  const kind = data.play.kind
  if (kind === "plugin-parse") {
    const proc = { data() { return { settings: {} } } }
    lilApi.call(proc, { gfm: true, allowDangerousHtml: true })
    return JSON.stringify(proc.parser(src), null, 2)
  }
  if (kind === "md-html") return lilApi(src, { gfm: true, allowDangerousHtml: true })
  if (kind === "md-json") {
    const tree = typeof lilApi === "function" && lilApi.length >= 1 ? lilApi(src, { gfm: true }) : lilApi
    if (tree && tree.type) return JSON.stringify(tree, null, 2)
    const proc = { data() { return { settings: {} } } }
    lilApi.call(proc, { gfm: true })
    return JSON.stringify(proc.parser(src), null, 2)
  }
  if (kind === "json-json") {
    const tree = JSON.parse(src)
    if (data.file === "remark-rehype") return JSON.stringify(lilApi.call({})(tree), null, 2)
    return JSON.stringify(lilApi(tree), null, 2)
  }
  if (kind === "json-html") {
    if (data.file === "rehype-stringify") {
      const host = {}
      lilApi.call(host, { allowDangerousHtml: true })
      return host.compiler(JSON.parse(src))
    }
    return lilApi(JSON.parse(src))
  }
  if (kind === "md-md") {
    const proc = lilApi()
    if (data.play.gfm) proc.data("settings", { gfm: true })
    const file = proc.processSync(src)
    return String(file.value ?? file.result ?? "")
  }
  if (kind === "html-html") return String(lilApi().processSync(src).value ?? "")
  if (kind === "unified") {
    const proc = lilApi().use(function headingPlugin() {
      this.parser = (doc) => ({
        type: "root",
        children: String(doc)
          .split(/\n+/)
          .filter(Boolean)
          .map((value) => ({ type: "paragraph", children: [{ type: "text", value }] })),
      })
      this.compiler = (tree) => (tree.children ?? []).map((node) => node.children?.[0]?.value ?? "").join("\n")
      return (tree) => {
        for (const node of tree.children ?? []) {
          const text = node.children?.[0]
          if (text && String(text.value).startsWith("# ")) text.value = String(text.value).slice(2).toUpperCase()
        }
        return tree
      }
    })
    return String(proc.processSync(src).result ?? proc.processSync(src).value ?? "")
  }
  if (kind === "tex-html") return lilApi(src, { displayMode: true, throwOnError: false })
  if (kind === "tex-hast") {
    const tree = {
      type: "root",
      children: [
        {
          type: "element",
          tagName: "span",
          properties: { className: ["math-inline"] },
          children: [{ type: "text", value: src }],
        },
      ],
    }
    const out = (typeof lilApi === "function" ? lilApi({ throwOnError: false }) : lilApi)(tree)
    const walk = (node) => {
      if (!node) return ""
      if (typeof node === "string") return node
      if (node.type === "raw" || node.type === "text") return node.value ?? ""
      if (Array.isArray(node.children)) return node.children.map(walk).join("")
      return ""
    }
    return walk(out)
  }
  if (kind === "md-plugin") {
    const parse = pick(awaitImportCache, "remarkParse")
    const proc = parseHost(lilApi, data.play.pluginOptions ?? {})
    parse.call(proc, {})
    let tree = proc.parser(src)
    if (data.play.transform !== false) {
      const transform = lilApi.call(proc, data.play.pluginOptions ?? {})
      if (typeof transform === "function") tree = transform(tree)
    }
    return JSON.stringify({ settings: proc.data().settings, tree }, null, 2)
  }
  return String(src)
}

let awaitImportCache = null
if (data.play.kind === "md-plugin") {
  awaitImportCache = await import("./vendor/remark-parse.js")
}

function runOfficial(src) {
  if (!officialApi) throw new Error("official lane is not on this page")
  const kind = data.play.kind
  if (kind === "plugin-parse") {
    const proc = {
      data() {
        return { settings: {} }
      },
    }
    officialApi.call(proc)
    return JSON.stringify(proc.parser(src), null, 2)
  }
  if (kind === "md-html") return officialApi(src)
  if (kind === "md-json") return JSON.stringify(officialApi(src), null, 2)
  if (kind === "json-json") return JSON.stringify(officialApi(JSON.parse(src)), null, 2)
  if (kind === "json-html") return officialApi(JSON.parse(src))
  if (kind === "md-md") return String(officialApi().processSync(src))
  if (kind === "html-html") return String(officialApi().processSync(src))
  if (kind === "tex-html") return officialApi.renderToString ? officialApi.renderToString(src, { displayMode: true, throwOnError: false }) : officialApi(src)
  if (kind === "unified") {
    const file = officialApi().use(function () {
      this.Parser = (doc) => ({ type: "root", children: [{ type: "text", value: doc }] })
      this.Compiler = (tree) => tree.children[0].value
    }).processSync(src)
    return String(file)
  }
  return runLil(src)
}

function renderPreview() {
  const src = document.querySelector("#source").value
  const out = document.querySelector("#output")
  const frame = document.querySelector("#preview")
  try {
    const result = currentEngine() === "official" ? runOfficial(src) : runLil(src)
    const htmlKinds = new Set(["md-html", "json-html", "html-html", "tex-html", "tex-hast"])
    if (htmlKinds.has(data.play.kind)) {
      out.hidden = true
      frame.hidden = false
      writeFrame(result)
    } else {
      frame.hidden = true
      out.hidden = false
      out.textContent = result
    }
  } catch (error) {
    frame.hidden = true
    out.hidden = false
    out.textContent = String(error && error.message ? error.message : error)
  }
}

function bindPlayground() {
  const source = document.querySelector("#source")
  source.value = data.play.sample
  source.addEventListener("input", renderPreview)
  for (const input of document.querySelectorAll("input[name=engine]")) {
    input.addEventListener("change", renderPreview)
  }
  const samples = document.querySelector("#samples")
  for (const item of data.play.samples ?? []) {
    const button = document.createElement("button")
    button.type = "button"
    button.textContent = item.label
    button.addEventListener("click", () => {
      source.value = item.value
      renderPreview()
    })
    samples.append(button)
  }
  const race = document.querySelector("#race")
  if (!officialApi) {
    race.hidden = true
    const officialRadio = document.querySelector("input[name=engine][value=official]")
    if (officialRadio) officialRadio.closest("label").hidden = true
  }
  race.addEventListener("click", () => {
    const src = source.value
    const loops = 10
    const run = (fn) => {
      fn(src)
      const start = performance.now()
      for (let i = 0; i < loops; i++) fn(src)
      return performance.now() - start
    }
    const lilMs = run(runLil)
    const officialMs = officialApi ? run(runOfficial) : null
    document.querySelector("#race-out").textContent = officialMs
      ? `${data.package} ${lilMs.toFixed(1)} ms · ${data.pin} ${officialMs.toFixed(1)} ms · ${fasterThan(lilMs, officialMs).text}`
      : `${data.package} ${lilMs.toFixed(1)} ms`
  })
  renderPreview()
}

renderHero()
renderPerf()
renderSize()
bindCopy()
bindProgress()
bindPlayground()
