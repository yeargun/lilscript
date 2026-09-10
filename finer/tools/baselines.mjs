// The upstream baselines table, shared by every tool that judges a port.
//
// Extracted from fleet.mjs so an A/B tool no longer has to scrape it with a
// regex: finer/out/048/fleet-compare.mjs did exactly that, and a table entry
// whose spelling drifted would silently drop the port from the comparison
// rather than fail it.
//
// `repo` is passed in because these paths are resolved relative to the
// checkout, and the callers already know where they are.
import { join } from "node:path"

export function baselines(repo) {
  return {

    jquerylil: { artifact: "dist/jquery.esm.js", upstream: join(repo, "benchmarks/popular/node_modules/jquery/dist/jquery.min.js") },
    markedlil: { artifact: "dist/marked.esm.js", upstream: join(repo, "benchmarks/popular/node_modules/marked/marked.min.js") },
    mobxlil: { artifact: "dist/mobx.esm.js", upstream: join(repo, "benchmarks/popular/node_modules/mobx/dist/mobx.esm.production.min.js") },
    motionlil: { artifact: "dist/full.js", upstream: join(repo, "benchmarks/popular/node_modules/motion/dist/motion.js") },
    // posthoglil's bar is the official kernel through Vite 8 / Oxc with mangling on,
    // measured by the port's own site harness with this same codec (site/results.json,
    // row `kernel-oxc-mangle`). Terser lands one byte behind it at 5626, so Oxc is the
    // one to beat.
    posthoglil: { artifact: "dist/posthog.esm.js", terserBrotli: 5622 },
    // zodlil ships a closer-world build as its primary, compared against the official
    // graph minified by Terser with mangling on (site/results.json, `official-terser-mangle`).
    zodlil: { artifact: "dist/zod.core.js", terserBrotli: 52561 },
    katexlil: { artifact: "dist/katex.esm.js", terserBrotli: 63137 },
    micromarklil: { artifact: "dist/micromark.esm.js", terserBrotli: 22776 },
    "mdast-util-from-markdownlil": { artifact: "dist/from-markdown.esm.js", terserBrotli: 23279 },
    "mdast-util-to-hastlil": { artifact: "dist/to-hast.esm.js", terserBrotli: 5016 },
    "hast-util-to-htmllil": { artifact: "dist/to-html.esm.js", terserBrotli: 9839 },
    "remark-parselil": { artifact: "dist/remark-parse.esm.js", terserBrotli: 23283 },
    "remark-rehypelil": { artifact: "dist/remark-rehype.esm.js", terserBrotli: 5061 },
    "remark-gfmlil": { artifact: "dist/remark-gfm.esm.js", terserBrotli: 11238 },
    // 8.73: rebuilt like-for-like on 2026-09-10 and the pinned 2150 was *generous
    // to us* by 53 -- corrected against our own interest, for the same reason
    // react-markdownlil's was corrected in its favour: a bar nobody can
    // reconstruct is not a bar. Upstream `remark-math` from npm, esbuild
    // --bundle --format=esm (our artifact imports nothing, so neither does the
    // bar), then Terser `-c passes=3 -m --module` -- compress defaults, since
    // 013 records the baseline has `pure_getters` off and `unsafe` is not
    // eligible. 2,097 Brotli under lilscript-codec against our 2,248; scopes
    // verified equal, one named export each, mathFlow 29 occurrences both
    // sides, mathText 17. The gap is +151, not +98.
    "remark-mathlil": { artifact: "dist/remark-math.esm.js", terserBrotli: 2097 },
    "remark-breakslil": { artifact: "dist/remark-breaks.esm.js", terserBrotli: 1198 },
    "rehype-stringifylil": { artifact: "dist/rehype-stringify.esm.js", terserBrotli: 9886 },
    rehypelil: { artifact: "dist/rehype.esm.js", terserBrotli: 55080 },
    remarklil: { artifact: "dist/remark.esm.js", terserBrotli: 32551 },
    unifiedlil: { artifact: "dist/unified.esm.js", terserBrotli: 4425 },
    "rehype-katexlil": { artifact: "dist/rehype-katex.esm.js", terserBrotli: 113063 },
    // 8.68/8.73: the previous 31092 compared different programs -- status.md
    // flagged it ("the committed artifact was React-external glue and the tree
    // inlines it") and it was never re-derived after the port grew to bundle the
    // markdown stack. Rebuilt like-for-like 2026-09-10: upstream `react-markdown`
    // from npm, esbuild --bundle --format=esm with `react` and `react/jsx-runtime`
    // external (exactly what our artifact imports), then Terser `-c passes=3 -m
    // --module`. Compress defaults deliberately: 013 records that the baseline
    // has `pure_getters` off, and `unsafe` is not eligible -- with both on this
    // reads 40,297, so the construction is worth ~50 bytes and has to be stated.
    // 144,305 raw / 40,348 Brotli under lilscript-codec against our 41,721.
    "react-markdownlil": { artifact: "dist/react-markdown.esm.js", terserBrotli: 40348 },
  }
}
