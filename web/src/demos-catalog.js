import motionLab from "./motion-lab-results.json" with { type: "json" };
import popularData from "./popular-library-results.json" with { type: "json" };
import libraryData from "./library-results.json" with { type: "json" };
import pairedData from "./paired-results.json" with { type: "json" };
import algorithmData from "./algorithm-demo-results.json" with { type: "json" };
import scenarioData from "./scenario-results.json" with { type: "json" };
import clientRuntime from "./client-runtime-results.json" with { type: "json" };
import apiParity from "./solid-api-parity.json" with { type: "json" };
import lsxParity from "./solid-lsx-parity.json" with { type: "json" };

import previewMap from "./demo-preview-map.json" with { type: "json" };

export const demoGroups = [
  { id: "apps", title: "Applications" },
  { id: "solidlil", title: "SolidLil" },
  { id: "motion", title: "Motion" },
  { id: "libraries", title: "Libraries" },
  { id: "algorithms", title: "Algorithms" },
];

const githubTree = "https://github.com/yeargun/lilscript/tree/main";

const popularBlurbs = {
  nanoid: "Tiny ID generator. Published browser entrypoint.",
  mitt: "Small event emitter. Same on/off/emit contract.",
  clsx: "Class-name builder. Brotli size gate is currently open.",
  immer: "Immutable updates through produce. Candidate surface.",
  "redux-toolkit": "Store core subset, not the full toolkit.",
  zod: "Schema parse subset, not the full Zod surface.",
  acorn: "Parser subset used by the contract app.",
  preact: "Core render subset. Candidate surface.",
  "gl-matrix": "Vector and matrix math. Exact browser entrypoint.",
  motion: "mix, wrap, stagger, and spring. Not the DOM package.",
  jquery: "Full-library size row, not a tree-shaken app.",
};

function titleCase(id) {
  return id
    .split("-")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function github(path) {
  return { href: `${githubTree}/${path}`, label: "Source on GitHub" };
}

function pageUrl(url) {
  if (!url) return null;
  const hashIndex = url.indexOf("#");
  const hash = hashIndex === -1 ? "" : url.slice(hashIndex);
  const withoutHash = hashIndex === -1 ? url : url.slice(0, hashIndex);
  const [path, query] = withoutHash.split("?");
  const normalized = path.endsWith(".html")
    ? path
    : `${path.endsWith("/") ? path : `${path}/`}index.html`;
  return `${normalized}${query ? `?${query}` : ""}${hash}`;
}

function pick(list, id) {
  return list?.find((item) => item.id === id) ?? null;
}

function artifactSizes(artifact) {
  return artifact
    ? { raw: artifact.raw, gzip: artifact.gzip, brotli: artifact.brotli }
    : null;
}

function sizes(value) {
  if (!value || value.raw === "—" || !Number.isFinite(value.brotli)) return null;
  return { raw: value.raw, gzip: value.gzip, brotli: value.brotli };
}

function brotliRatio(candidate, baseline) {
  if (
    !Number.isFinite(candidate) ||
    !Number.isFinite(baseline) ||
    baseline === 0
  ) {
    return null;
  }
  return candidate / baseline;
}

function mean(values) {
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function attachUrls(pane, mappedUrl) {
  return { ...pane, url: pageUrl(mappedUrl ?? pane?.url) };
}

function attachPreview(demo) {
  const mapped = previewMap[demo.id] ?? {};
  const variants = (demo.variants ?? []).map((variant) => {
    const mappedVariant = previewMap[variant.id] ?? {};
    const baseline = attachUrls(
      { ...demo.baseline, ...variant.baseline },
      mappedVariant.baseline,
    );
    const candidate = attachUrls(
      { ...demo.candidate, ...variant.candidate },
      mappedVariant.candidate,
    );
    return {
      ...variant,
      baseline,
      candidate,
      source: variant.source ?? demo.source,
      kind: baseline.url && candidate.url ? "visual" : variant.kind ?? demo.kind,
    };
  });
  const baseline = attachUrls(demo.baseline, mapped.baseline);
  const candidate = attachUrls(demo.candidate, mapped.candidate);
  const visual =
    (baseline.url && candidate.url) ||
    variants.some((variant) => variant.kind === "visual");
  return {
    ...demo,
    kind: visual ? "visual" : demo.kind,
    baseline,
    candidate,
    variants,
    source: demo.source,
  };
}

function jsBaseline(artifacts) {
  const javascript = artifacts.filter(
    (item) => !String(item.id).startsWith("lilscript"),
  );
  return javascript.reduce((best, item) => {
    if (!best || item.brotli < best.brotli) return item;
    return best;
  }, null);
}

function lilCandidate(artifacts) {
  const preferred = ["lilscript-closed-world", "lilscript-vite-oxc", "lilscript"];
  for (const id of preferred) {
    const found = pick(artifacts, id);
    if (found) return found;
  }
  return artifacts.find((item) => String(item.id).startsWith("lilscript")) ?? null;
}

function solidSurface(id) {
  return (
    pick(clientRuntime.closedWorldSurfaces, id) ??
    pick(clientRuntime.surfaces, id) ??
    pick(clientRuntime.comparisons, id)
  );
}

function solidPair(id) {
  const surface = solidSurface(id);
  if (!surface) return null;
  return {
    surface,
    solid: pick(surface.artifacts, "solid"),
    solidlil: pick(surface.artifacts, "solidlil"),
  };
}

function codecNote() {
  return "Brotli-11 is the gated transfer size of the selected JavaScript. gzip-9 and raw describe that same artifact and may trade off.";
}

function pairRatio(baseline, candidate) {
  return brotliRatio(candidate?.brotli, baseline?.brotli);
}

const lastro = {
  id: "lastro",
  group: "apps",
  featured: true,
  kind: "visual",
  title: "Lastro",
  kicker: "Application",
  summary: "Parcel Market next to the Astro control. Same CSS and behavior.",
  baseline: {
    label: "Astro control",
    url: "/demos/lastro-astro/index.html",
    sizes: { raw: 8801, gzip: 3132, brotli: 2672 },
  },
  candidate: {
    label: "Lastro · Lilastro + Lilpack",
    url: "/marketplace.html?embed=1",
    sizes: { raw: 7957, gzip: 3132, brotli: 2665 },
  },
  source: github("web"),
  settings: { costModel: codecNote() },
};

const keyed = {
  id: "solidlil-keyed",
  group: "solidlil",
  featured: true,
  kind: "visual",
  title: "SolidLil keyed",
  kicker: "Closed-world app",
  summary: "js-framework-benchmark keyed table. Official Solid versus compiler-only SolidLil.",
  baseline: {
    label: "SolidJS keyed",
    url: "/demos/keyed-solid/index.html",
    sizes: { raw: 11563, gzip: 4810, brotli: 4358 },
  },
  candidate: {
    label: "SolidLil keyed",
    url: "/demos/keyed-solidlil/index.html",
    sizes: { raw: 11131, gzip: 4436, brotli: 3940 },
  },
  source: github("benchmarks/js-framework-benchmark/adapter"),
  settings: { costModel: codecNote() },
};

const lsxPair = solidPair("lsx-client-app");
const lsx = {
  id: "solidlil-lsx",
  group: "solidlil",
  featured: true,
  kind: "lab",
  title: "LSX vs JSX",
  kicker: "Client UI",
  summary: `${lsxParity.counts.runtimeVerified}/${lsxParity.counts.expected} in-scope client families. Hydration and SSR stay excluded.`,
  baseline: {
    label: "Solid JSX",
    sizes: artifactSizes(lsxPair?.solid),
  },
  candidate: {
    label: "SolidLil LSX",
    sizes: artifactSizes(lsxPair?.solidlil),
  },
  source: github("labs/solid-client"),
  settings: { costModel: codecNote() },
};

const api = {
  id: "solidlil-api",
  group: "solidlil",
  featured: true,
  kind: "lab",
  title: "SolidLil API",
  kicker: "Open-world ABI",
  summary: `${apiParity.totals.verified}/${apiParity.totals.expected} public exports verified against ${apiParity.baseline}.`,
  baseline: {
    label: "solid-js browser ESM",
    facts: apiParity.surfaces.map(
      (surface) => `${surface.name}: ${surface.counts.expected} exports`,
    ),
    sizes: artifactSizes(solidPair("core")?.solid),
  },
  candidate: {
    label: "SolidLil browser entries",
    facts: apiParity.surfaces.map(
      (surface) =>
        `${surface.name}: ${surface.counts.verified}/${surface.counts.expected} verified`,
    ),
    sizes: artifactSizes(solidPair("core")?.solidlil),
  },
  source: github("labs/solid-client/packages/solidlil"),
  settings: { costModel: codecNote() },
};

const motionFamilies = [
  {
    id: "motion-showcases",
    title: "Motion showcases",
    summary: "Carousel, sequence, spring, wave, and gesture scenes. Same DOM contract.",
    match: (example) => example.id.startsWith("showcase-"),
  },
  {
    id: "motion-animate",
    title: "Motion animate",
    summary: "Play, CSS variables, stagger, spring, and scroll call-sites.",
    match: (example) => example.id.startsWith("animate-"),
  },
  {
    id: "motion-interaction",
    title: "Motion interaction",
    summary: "Hover, press, in-view, resize, values, and a stagger stress fixture.",
    match: (example) =>
      !example.id.startsWith("showcase-") && !example.id.startsWith("animate-"),
  },
];

const motionDemos = motionFamilies.map((family) => {
  const examples = motionLab.examples.filter(family.match);
  const variants = examples.map((example) => ({
    id: `motion-${example.id}`,
    title: titleCase(example.id.replace(/^showcase-/, "")),
    baseline: {
      label: "npm motion@13",
      url: example.npmUrl,
      sizes: example.npm,
    },
    candidate: {
      label: "LilScript port",
      url: example.lilUrl,
      sizes: example.lil,
    },
    source: github(`lilastro/browser/${example.id}`),
  }));
  const ratios = examples.map((example) => example.brotliRatio);
  return {
    id: family.id,
    group: "motion",
    featured: family.id === "motion-showcases",
    kind: "visual",
    title: family.title,
    kicker: "npm motion vs LilScript",
    summary: family.summary,
    baseline: variants[0].baseline,
    candidate: variants[0].candidate,
    variants,
    ratio: mean(ratios),
    wins: examples.filter((example) => example.brotliRatio < 1).length,
    total: examples.length,
    source: github("lilastro/browser"),
    settings: { costModel: codecNote() },
  };
});

const popularDemos = popularData.results
  .filter((result) => result.id !== "solid-js")
  .map((result) => {
    const baseline = sizes(result.vite) ?? sizes(result.terser) ?? sizes(result.rawJs);
    const candidate = sizes(result.lilscriptVite) ?? sizes(result.lilscript);
    return {
      id: `lib-${result.id}`,
      group: "libraries",
      featured: false,
      kind: "lab",
      title: result.project,
      kicker: result.eligible ? "Eligible" : "Candidate",
      summary: popularBlurbs[result.id] ?? result.scope ?? result.project,
      baseline: { label: "npm + Vite 8", sizes: baseline },
      candidate: { label: "LilScript + Vite 8", sizes: candidate },
      source: github(`benchmarks/popular/ports/${result.id}`),
      settings: { costModel: codecNote() },
    };
  });

const seenPorts = new Set();
const completeDemos = [...libraryData.results, ...(libraryData.diagnostics ?? [])]
  .filter((result) => {
    if (seenPorts.has(result.id)) return false;
    seenPorts.add(result.id);
    return true;
  })
  .map((result) => {
    const vite = pick(result.surfaceArtifacts, "vite") ?? pick(result.artifacts, "vite");
    const lil = pick(result.surfaceArtifacts, "lilscript") ?? pick(result.artifacts, "lilscript");
    return {
      id: `port-${result.id}`,
      group: "libraries",
      featured: false,
      kind: "lab",
      title: result.title,
      kicker: result.eligible ? "Complete port" : "Port · gated",
      summary: result.scope,
      baseline: { label: "npm + Vite", sizes: artifactSizes(vite) },
      candidate: { label: "LilScript port", sizes: artifactSizes(lil) },
      source: github(`benchmarks/libraries/ports/${result.id}`),
      settings: { costModel: codecNote() },
    };
  });

const pairedVariants = pairedData.results.map((result) => ({
  id: `paired-${result.id}`,
  title: titleCase(result.id),
  baseline: { label: "Closure ADVANCED", sizes: result.closure },
  candidate: { label: "LilScript", sizes: result.lilscript },
  source: github("benchmarks/paired"),
}));

const pairedDemo = {
  id: "paired",
  group: "algorithms",
  featured: false,
  kind: "lab",
  title: "Paired compression",
  kicker: "Compiler cases",
  summary: "Same stdout contract. Closure ADVANCED JavaScript versus a Brotli-objective LilScript compile.",
  baseline: pairedVariants[0].baseline,
  candidate: pairedVariants[0].candidate,
  variants: pairedVariants,
  ratio: mean(
    pairedData.results.map((result) =>
      brotliRatio(result.lilscript.brotli, result.closure.brotli),
    ),
  ),
  wins: pairedData.results.filter(
    (result) => result.lilscript.brotli < result.closure.brotli,
  ).length,
  total: pairedData.results.length,
  source: github("benchmarks/paired"),
  settings: { costModel: codecNote() },
};

const algorithmVariants = algorithmData.cases.map((result) => ({
  id: `algo-${result.id}`,
  title: result.title,
  summary: result.hypothesis,
  baseline: {
    label: `${result.baseline.tool} · ${result.baseline.id}`,
    sizes: result.baseline,
  },
  candidate: { label: "LilScript Brotli lane", sizes: result.lilscript },
  source: github(`comparison/algorithms/cases/${result.id}`),
  passed: result.passed,
}));

const algorithmDemo = {
  id: "algorithms",
  group: "algorithms",
  featured: false,
  kind: "lab",
  title: "Algorithm corpus",
  kicker: "Typed pipelines",
  summary: "Independently minimized JavaScript baselines. One Brotli-objective LilScript lane per case.",
  baseline: algorithmVariants[0].baseline,
  candidate: algorithmVariants[0].candidate,
  variants: algorithmVariants,
  ratio: mean(
    algorithmData.cases.map((result) =>
      brotliRatio(result.lilscript.brotli, result.baseline.brotli),
    ),
  ),
  wins: algorithmData.cases.filter((result) => result.passed).length,
  total: algorithmData.cases.length,
  source: github("comparison/algorithms"),
  settings: { costModel: codecNote() },
};

const scenarioDemos = scenarioData.results.map((result) => {
  const baseline = jsBaseline(result.artifacts);
  const candidate = lilCandidate(result.artifacts);
  return {
    id: `app-${result.id}`,
    group: "apps",
    featured: false,
    kind: "lab",
    title: result.title,
    kicker: "Closed-world app",
    summary: result.summary,
    baseline: { label: baseline.label, sizes: artifactSizes(baseline) },
    candidate: { label: candidate.label, sizes: artifactSizes(candidate) },
    source: github(`benchmarks/scenarios/apps/${result.id}`),
    settings: { costModel: codecNote() },
  };
});

export const demos = [
  lastro,
  keyed,
  lsx,
  api,
  ...scenarioDemos,
  ...motionDemos,
  ...popularDemos,
  ...completeDemos,
  pairedDemo,
  algorithmDemo,
].map((demo) => {
  const next = attachPreview(demo);
  if (Number.isFinite(next.ratio)) return next;
  return { ...next, ratio: pairRatio(next.baseline.sizes, next.candidate.sizes) };
});

export function resolveDemo(id) {
  const card = demos.find((demo) => demo.id === id);
  if (card) return { card, variant: card.variants[0] ?? null };
  for (const demo of demos) {
    const variant = demo.variants.find((item) => item.id === id);
    if (variant) return { card: demo, variant };
  }
  return { card: demos[0], variant: demos[0].variants[0] ?? null };
}

export function demoById(id) {
  const { card, variant } = resolveDemo(id);
  if (!variant || variant.id === card.id) return card;
  return {
    ...card,
    id: variant.id,
    title: variant.title,
    summary: variant.summary ?? card.summary,
    kind: variant.kind,
    baseline: variant.baseline,
    candidate: variant.candidate,
    source: variant.source ?? card.source,
    facts: variant.candidate?.facts ?? card.candidate.facts,
  };
}

export function allDemoPairs() {
  return demos.flatMap((demo) =>
    demo.variants.length > 0
      ? demo.variants.map((variant) => demoById(variant.id))
      : [demo],
  );
}
