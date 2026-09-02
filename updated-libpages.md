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

None yet. Local verification is not recorded as publication.

## Release queue

| Library | Local build | Local Pages gate | Repository | GitHub Pages | Notes |
| --- | --- | --- | --- | --- | --- |
| hast-util-to-htmllil | ready | passed | pending | pending | Size receipts refreshed. |
| jquerylil | blocked | blocked | pending | pending | Feature compiler selected syntactically invalid JavaScript; compiler fix required. |
| katexlil | ready | passed | pending | pending | Size receipts refreshed. |
| markedlil | ready | passed | pending | pending | Raw/gzip/Brotli lanes and hero metrics refreshed. |
| mdast-util-from-markdownlil | ready | passed | pending | pending | Size receipts refreshed. |
| mdast-util-to-hastlil | ready | passed | pending | pending | Size receipts refreshed. |
| micromarklil | ready | passed | pending | pending | Size receipts refreshed. |
| mobxlil | ready | passed | pending | pending | Current Vite comparison refreshed; specialized official production lane remains smaller. |
| monacolil | ready | passed | pending | pending | 113-folder/994-conversion report regenerated. |
| motionlil | ready | passed | pending | pending | Full test, type, Pages, and package gates passed. |
| playcanvaslil | ready | passed | pending | pending | Compiler provenance repinned; page reports the open-world loss and closed-world win. |
| posthoglil | ready | passed | pending | pending | Core plus five independently measured packs refreshed. |
| react-markdownlil | ready | passed | pending | pending | Size receipts and stack dashboard refreshed. |
| rehype-katexlil | ready | passed | pending | pending | Size receipts refreshed. |
| rehype-stringifylil | ready | passed | pending | pending | Size receipts refreshed. |
| rehypelil | ready | passed | pending | pending | Size receipts refreshed. |
| remark-breakslil | ready | passed | pending | pending | Rebuilt; existing receipt remains current. |
| remark-gfmlil | ready | passed | pending | pending | Size receipts refreshed. |
| remark-mathlil | ready | passed | pending | pending | Size receipts refreshed. |
| remark-parselil | ready | passed | pending | pending | Size receipts refreshed. |
| remark-rehypelil | ready | passed | pending | pending | Size receipts refreshed. |
| remarklil | ready | passed | pending | pending | Size receipts refreshed. |
| solidlil | in progress | pending | pending | pending | Full demo/JFB/performance/Pages regeneration is running. |
| unifiedlil | ready | passed | pending | pending | Size receipts refreshed. |
| zodlil | ready | passed | pending | pending | Size plus fresh Chromium and Node benchmark lanes refreshed. |

## Publication record format

Each completed entry records the repository commit, Pages workflow run, live
URL, package name/version, and the verification command that passed. This file
is updated only after the live deployment is confirmed.
