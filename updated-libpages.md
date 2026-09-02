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

Compiler: `feature/source-maps` at `f504c93` (the Oxc terminal-parser admission gate,
`4e799a8`, was added afterwards for jquerylil and is not what built these).
Verification that passed locally before each push: `npm run check:site`; after the
Pages workflow finished, the live `index.html` was fetched and compared byte-for-byte
with the local `_site/index.html`. Pushed 2026-09-02 15:23 UTC; all deployments were
live by 15:35 UTC. npm publication is a separate step (see the queue note below).

| Library | Commit | Pages run | Live URL | Package |
| --- | --- | --- | --- | --- |
| hast-util-to-htmllil | `f682d3c` | [33648317408](https://github.com/yeargun/hast-util-to-htmllil/actions/runs/33648317408) | https://yeargun.github.io/hast-util-to-htmllil/ | `@itslil/hast-util-to-html@9.0.7` |
| katexlil | `a8be5af` | [33654585183](https://github.com/yeargun/katexlil/actions/runs/33654585183) | https://yeargun.github.io/katexlil/ | `@itslil/katex@0.16.24` |
| markedlil | `0c3ac40` | [33648261219](https://github.com/yeargun/markedlil/actions/runs/33648261219) | https://yeargun.github.io/markedlil/ | `@itslil/marked@18.0.14` |
| mdast-util-from-markdownlil | `6635a35` | [33648329657](https://github.com/yeargun/mdast-util-from-markdownlil/actions/runs/33648329657) | https://yeargun.github.io/mdast-util-from-markdownlil/ | `@itslil/mdast-util-from-markdown@2.0.4` |
| mdast-util-to-hastlil | `1c0fd2c` | [33648334353](https://github.com/yeargun/mdast-util-to-hastlil/actions/runs/33648334353) | https://yeargun.github.io/mdast-util-to-hastlil/ | `@itslil/mdast-util-to-hast@13.2.1` |
| micromarklil | `900d8b0` | [33648339678](https://github.com/yeargun/micromarklil/actions/runs/33648339678) | https://yeargun.github.io/micromarklil/ | `@itslil/micromark@4.0.3` |
| mobxlil | `d26997f` | [33648269808](https://github.com/yeargun/mobxlil/actions/runs/33648269808) | https://yeargun.github.io/mobxlil/ | `@itslil/mobx@7.0.1` |
| monacolil | `e7d810b` | [33648345309](https://github.com/yeargun/monacolil/actions/runs/33648345309) | https://yeargun.github.io/monacolil/ | `@itslil/monaco-editor@0.56.1` |
| motionlil | `de14494` | [33648278088](https://github.com/yeargun/motionlil/actions/runs/33648278088) | https://yeargun.github.io/motionlil/ | `motionlil@0.1.5` |
| posthoglil | `5392877` | [33648353478](https://github.com/yeargun/posthoglil/actions/runs/33648353478) | https://yeargun.github.io/posthoglil/ | `@itslil/posthog-js@1.418.11` |
| react-markdownlil | `0a91fc1` | [33648360443](https://github.com/yeargun/react-markdownlil/actions/runs/33648360443) | https://yeargun.github.io/react-markdownlil/ | `@itslil/react-markdown@10.1.0` |
| rehype-katexlil | `cf4be45` | [33648365921](https://github.com/yeargun/rehype-katexlil/actions/runs/33648365921) | https://yeargun.github.io/rehype-katexlil/ | `@itslil/rehype-katex@7.0.3` |
| rehype-stringifylil | `6f258ef` | [33648369019](https://github.com/yeargun/rehype-stringifylil/actions/runs/33648369019) | https://yeargun.github.io/rehype-stringifylil/ | `@itslil/rehype-stringify@10.0.3` |
| rehypelil | `8580514` | [33648372276](https://github.com/yeargun/rehypelil/actions/runs/33648372276) | https://yeargun.github.io/rehypelil/ | `@itslil/rehype@13.0.4` |
| remark-breakslil | `6c38bf6` | [33648375850](https://github.com/yeargun/remark-breakslil/actions/runs/33648375850) | https://yeargun.github.io/remark-breakslil/ | `@itslil/remark-breaks@4.0.2` |
| remark-gfmlil | `de1c639` | [33648378732](https://github.com/yeargun/remark-gfmlil/actions/runs/33648378732) | https://yeargun.github.io/remark-gfmlil/ | `@itslil/remark-gfm@4.0.3` |
| remark-mathlil | `ade9029` | [33648383672](https://github.com/yeargun/remark-mathlil/actions/runs/33648383672) | https://yeargun.github.io/remark-mathlil/ | `@itslil/remark-math@6.0.2` |
| remark-parselil | `986f324` | [33648389176](https://github.com/yeargun/remark-parselil/actions/runs/33648389176) | https://yeargun.github.io/remark-parselil/ | `@itslil/remark-parse@11.0.2` |
| remark-rehypelil | `dccf1c2` | [33648392442](https://github.com/yeargun/remark-rehypelil/actions/runs/33648392442) | https://yeargun.github.io/remark-rehypelil/ | `@itslil/remark-rehype@11.1.4` |
| remarklil | `d6ca47e` | [33648398129](https://github.com/yeargun/remarklil/actions/runs/33648398129) | https://yeargun.github.io/remarklil/ | `@itslil/remark@15.0.2` |
| solidlil | `f12de97` | [33648292868](https://github.com/yeargun/solidlil/actions/runs/33648292868) | https://yeargun.github.io/solidlil/ | `@itslil/solidjs@0.1.1` |
| unifiedlil | `80a4812` | [33648402814](https://github.com/yeargun/unifiedlil/actions/runs/33648402814) | https://yeargun.github.io/unifiedlil/ | `@itslil/unified@11.0.6` |
| zodlil | `32de544` | [33648407962](https://github.com/yeargun/zodlil/actions/runs/33648407962) | https://yeargun.github.io/zodlil/ | `@itslil/zod@4.4.4` |

## Release queue

| Library | Local build | Local Pages gate | Repository | GitHub Pages | Notes |
| --- | --- | --- | --- | --- | --- |
| jquerylil | rebuilt, held | passed | held | unchanged | Built on `4e799a8` (Oxc gate) so the artifact is now valid JavaScript and the 6 compat tests pass, but it is +359 Brotli against the shipped `30c000d` build (28225 → 28584 on `jquery.esm.js`): the gate refuses the smaller, malformed leaf the level-15 search preferred. Both builds still throw on `$(el).scrollTop(1)` (the arrow-spelled `this` method, finer 042 lead 1). Working tree left dirty; release only once a build beats the shipped size. |
| playcanvaslil | miscompiled, reverted | passed | `93b94c6` (revert to `9466f38`) | pending | The `f504c93` artifact fails `test:differential`: `ShaderProcessorGLSL.extract("#version 450\n attribute vec3 position;;\nvarying highp vec2 uv;\nuniform vec4 tint;\n…")` returns empty `attributes`/`varyings`/`uniforms` where upstream finds them. `check:site` does not exercise this; CI (`npm run check`) does. Reverted on `main` so the 2026-08-29 deployment (compiler `bcef1c4`) stays live. Needs a hypothesis folder before it is rebuilt. |

## Publication record format

Each completed entry records the repository commit, Pages workflow run, live
URL, package name/version, and the verification command that passed. This file
is updated only after the live deployment is confirmed.
