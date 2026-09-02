# Updated library Pages

This is the release ledger for rebuilding the maintained LilScript libraries,
publishing their repository changes, and refreshing their GitHub Pages sites.

A library belongs in **Published and live** only after all of these are true:

1. its release artifacts and comparison receipts were rebuilt from the pinned
   `feature/source-maps` compiler;
2. its local package and Pages gates passed;
3. its repository commit was pushed to `main`;
4. the resulting GitHub Pages workflow completed successfully; and
5. the live `yeargun.github.io` URL serves that deployment.

## Published and live

Compiler: `feature/source-maps` at `f504c93`. (The Oxc terminal-parser admission gate,
`4e799a8`, was added afterwards for jquerylil and did not build these; zodlil's core
reproduced byte-identically under it.) Every row: `npm run check:site` passed locally,
the commit was pushed to `main` on 2026-09-02, the Pages workflow linked below succeeded,
and the live `index.html` was fetched and compared byte-for-byte with the local
`_site/index.html`. "npm gate" is `npm run prepublishOnly` run locally on 2026-09-02 with
`LILSCRIPT_COMPILER`/`LILSCRIPT_CODEC` pointed at the feature build; a green gate means
`npm publish` will get through its own checks.

| Library | Commit | Pages run | Live URL | Package | npm gate |
| --- | --- | --- | --- | --- | --- |
| hast-util-to-htmllil | `f682d3c` | [33648317408](https://github.com/yeargun/hast-util-to-htmllil/actions/runs/33648317408) | https://yeargun.github.io/hast-util-to-htmllil/ | `@itslil/hast-util-to-html@9.0.7` | green |
| katexlil | `81075a5` | [33682137267](https://github.com/yeargun/katexlil/actions/runs/33682137267) | https://yeargun.github.io/katexlil/ | `@itslil/katex@0.16.24` | green |
| markedlil | `0c3ac40` | [33648261219](https://github.com/yeargun/markedlil/actions/runs/33648261219) | https://yeargun.github.io/markedlil/ | `@itslil/marked@18.0.14` | green |
| mdast-util-from-markdownlil | `6635a35` | [33648329657](https://github.com/yeargun/mdast-util-from-markdownlil/actions/runs/33648329657) | https://yeargun.github.io/mdast-util-from-markdownlil/ | `@itslil/mdast-util-from-markdown@2.0.4` | green |
| mdast-util-to-hastlil | `1c0fd2c` | [33648334353](https://github.com/yeargun/mdast-util-to-hastlil/actions/runs/33648334353) | https://yeargun.github.io/mdast-util-to-hastlil/ | `@itslil/mdast-util-to-hast@13.2.1` | green |
| micromarklil | `900d8b0` | [33648339678](https://github.com/yeargun/micromarklil/actions/runs/33648339678) | https://yeargun.github.io/micromarklil/ | `@itslil/micromark@4.0.3` | green |
| mobxlil | `d26997f` | [33648269808](https://github.com/yeargun/mobxlil/actions/runs/33648269808) | https://yeargun.github.io/mobxlil/ | `@itslil/mobx@7.0.1` | green |
| monacolil | `e7d810b` | [33648345309](https://github.com/yeargun/monacolil/actions/runs/33648345309) | https://yeargun.github.io/monacolil/ | `@itslil/monaco-editor@0.56.1` | green |
| motionlil | `de14494` | [33648278088](https://github.com/yeargun/motionlil/actions/runs/33648278088) | https://yeargun.github.io/motionlil/ | `motionlil@0.1.5` | green |
| posthoglil | `5392877` | [33648353478](https://github.com/yeargun/posthoglil/actions/runs/33648353478) | https://yeargun.github.io/posthoglil/ | `@itslil/posthog-js@1.418.11` | no `prepublishOnly` script (`npm pack` only) |
| react-markdownlil | `2ee2b53` | [33652678139](https://github.com/yeargun/react-markdownlil/actions/runs/33652678139) | https://yeargun.github.io/react-markdownlil/ | `@itslil/react-markdown@10.1.0` | green |
| rehype-katexlil | `f6ff62c` | [33652566344](https://github.com/yeargun/rehype-katexlil/actions/runs/33652566344) | https://yeargun.github.io/rehype-katexlil/ | `@itslil/rehype-katex@7.0.3` | green |
| rehype-stringifylil | `6f258ef` | [33648369019](https://github.com/yeargun/rehype-stringifylil/actions/runs/33648369019) | https://yeargun.github.io/rehype-stringifylil/ | `@itslil/rehype-stringify@10.0.3` | green |
| rehypelil | `8580514` | [33648372276](https://github.com/yeargun/rehypelil/actions/runs/33648372276) | https://yeargun.github.io/rehypelil/ | `@itslil/rehype@13.0.4` | green |
| remark-breakslil | `6c38bf6` | [33648375850](https://github.com/yeargun/remark-breakslil/actions/runs/33648375850) | https://yeargun.github.io/remark-breakslil/ | `@itslil/remark-breaks@4.0.2` | green |
| remark-gfmlil | `de1c639` | [33648378732](https://github.com/yeargun/remark-gfmlil/actions/runs/33648378732) | https://yeargun.github.io/remark-gfmlil/ | `@itslil/remark-gfm@4.0.3` | green |
| remark-mathlil | `ade9029` | [33648383672](https://github.com/yeargun/remark-mathlil/actions/runs/33648383672) | https://yeargun.github.io/remark-mathlil/ | `@itslil/remark-math@6.0.2` | green |
| remark-parselil | `986f324` | [33648389176](https://github.com/yeargun/remark-parselil/actions/runs/33648389176) | https://yeargun.github.io/remark-parselil/ | `@itslil/remark-parse@11.0.2` | green |
| remark-rehypelil | `dccf1c2` | [33648392442](https://github.com/yeargun/remark-rehypelil/actions/runs/33648392442) | https://yeargun.github.io/remark-rehypelil/ | `@itslil/remark-rehype@11.1.4` | green |
| remarklil | `d6ca47e` | [33648398129](https://github.com/yeargun/remarklil/actions/runs/33648398129) | https://yeargun.github.io/remarklil/ | `@itslil/remark@15.0.2` | green |
| solidlil | `bd8c8de` | [33659696126](https://github.com/yeargun/solidlil/actions/runs/33659696126) | https://yeargun.github.io/solidlil/ | `@itslil/solidjs@0.1.1` | green |
| unifiedlil | `80a4812` | [33648402814](https://github.com/yeargun/unifiedlil/actions/runs/33648402814) | https://yeargun.github.io/unifiedlil/ | `@itslil/unified@11.0.6` | **red**: WIP vfile port; `test/vfile.test.mjs` fails because `file.message()` returns a plain `Error`, not a `VFileMessage`. Finish the port or publish with `--ignore-scripts`. |
| zodlil | `69a48e0` | [33657444661](https://github.com/yeargun/zodlil/actions/runs/33657444661) | https://yeargun.github.io/zodlil/ | `@itslil/zod@4.4.4` | green |

Follow-up commits after the rebuild push (all deployed and verified live the same way):

- katexlil `0b76097`: `audit-parity.mjs` pins the upstream API version (0.16.22) instead of the
  package version and falls back to `node_modules/katex` when `/tmp/opencode/markdown-upstreams/katex`
  is absent. (`a8be5af` on top is another agent's site-measurement commit; its Pages run also succeeded.)
- react-markdownlil `2ee2b53`: `scripts/source-graph.mjs` sibling pins moved to remark-parse 11.0.2 and
  remark-rehype 11.1.4; `source-graph.lock.json` regenerated.
- rehype-katexlil `f6ff62c`: audit test and artifact banner pinned to 7.0.3.
- zodlil `69a48e0`: `dist/index.cjs` regenerated from the rebuilt `zod.core.js` (it was still the
  2026-08-28 bundle). `vendor/zod` must be a full clone at `e516c3b`; the script's blob-less HTTPS clone
  cannot fetch objects under npm on this host.
- solidlil `bd8c8de`: `apps-e2e` and `jfb` tests fall back to Playwright's Chromium when macOS Chrome is
  absent (`SOLIDLIL_CHROME` overrides); the jfb suite used to hang forever on a failed launch.

## Release queue

| Library | Local build | Local Pages gate | Repository | GitHub Pages | Notes |
| --- | --- | --- | --- | --- | --- |
| jquerylil | rebuilt, held | passed | held (tree dirty on `30c000d`) | unchanged | Built on `4e799a8` (Oxc gate): the artifact is now valid JavaScript and the 6 compat tests pass, but it is +359 Brotli against the shipped `30c000d` build (28225 → 28584 on `jquery.esm.js`) because the gate refuses the smaller, malformed leaf the level-15 search preferred. Both builds still throw on `$(el).scrollTop(1)` (the arrow-spelled `this` method, finer 042 lead 1). Release only once a build beats the shipped size. |
| playcanvaslil | miscompiled, reverted | passed | `93b94c6` (revert to `9466f38`), CI green | 2026-08-29 deployment still live | The `f504c93` artifact fails `test:differential`: `ShaderProcessorGLSL.extract("#version 450\n attribute vec3 position;;\nvarying highp vec2 uv;\nuniform vec4 tint;\n…")` returns empty `attributes`/`varyings`/`uniforms` where upstream finds them. `check:site` does not exercise this; CI (`npm run check`) does, and it also rebuilds with the pinned compiler, so the artifact under `bcef1c4` is what is live. Needs a hypothesis folder before it is rebuilt. |

## Publication record format

Each completed entry records the repository commit, Pages workflow run, live
URL, package name/version, and the verification command that passed. This file
is updated only after the live deployment is confirmed.
