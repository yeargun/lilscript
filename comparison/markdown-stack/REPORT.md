# Markdown stack measurement

Generated: 2026-08-30T23:40:48.934Z

Manifest SHA-256: `fcdb386bb629e806dc0ac2e3e786f3d77176b9c369aceb37644eda0cb99cbf71`

## Canonical comparison

| Port | Official Terser raw | Official Terser gzip | Official Terser Brotli | Lil raw | Lil gzip | Lil Brotli | Brotli delta | Result |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| micromark | 81530 | 26383 | 22776 | 105681 | 32134 | 27344 | +4568 | loss |
| mdast-util-from-markdown | 84681 | 27038 | 23279 | 94541 | 31481 | 26852 | +3573 | loss |
| remark-parse | 84866 | 27101 | 23283 | 95544 | 31765 | 27021 | +3738 | loss |
| remark | 119872 | 37366 | 32551 | 205300 | 53926 | 45880 | +13329 | loss |
| unified | 13579 | 4862 | 4425 | 21316 | 6028 | 5409 | +984 | loss |
| mdast-util-to-hast | 17117 | 5537 | 5016 | 14620 | 4762 | 4290 | -726 | win |
| hast-util-to-html | 31882 | 11235 | 9839 | 30253 | 9962 | 8811 | -1028 | win |
| remark-rehype | 17263 | 5595 | 5061 | 15027 | 4911 | 4390 | -671 | win |
| rehype | 221625 | 67746 | 55080 | 372587 | 78982 | 64992 | +9912 | loss |
| rehype-stringify | 31975 | 11269 | 9886 | 30572 | 10302 | 9141 | -745 | win |
| remark-gfm | 42343 | 12446 | 11238 | 38873 | 13144 | 11617 | +379 | loss |
| remark-breaks | 3045 | 1299 | 1198 | 2746 | 1258 | 1131 | -67 | win |
| remark-math | 6442 | 2378 | 2150 | 7615 | 2887 | 2600 | +450 | loss |
| katex | 267745 | 76650 | 63137 | 295886 | 83961 | 69669 | +6532 | loss |
| rehype-katex | 474237 | 138812 | 113063 | 586338 | 104850 | 84501 | -28562 | win |
| react-markdown | 117759 | 34914 | 31092 | 216436 | 56636 | 47861 | +16769 | loss |
| **Total** | **1615961** | **490631** | **413074** | **2133335** | **526989** | **441509** | **+28435** | **6W / 10L / 0T** |

Positive delta means the standard Lil graph is larger. Results use Brotli.

## Canonical artifact hashes

| Port | Official graph SHA-256 | Official Terser SHA-256 | Lil graph SHA-256 |
|---|---|---|---|
| micromark | `9e2048d4ef7fe32679e7149e16fc0c329c6d02b65f204a4ff71379e3b1cdaf9b` | `d2cbd64a7b310236b68ecaad941fc9eee542d4a6e9359c4434e450db6d19025f` | `defa94b263d71001d96f7789cb89e563a70ea37e6147b2c0459b2f34db1afc4d` |
| mdast-util-from-markdown | `7ac67e89ca522342d67e04d1d382f8759503052cc988b92f3320e3acf443b9d6` | `ed143c7678f0ecef0439c2d1e10934de7e79a73ea3337dc636725da9693337b8` | `21bb82d4ff2023d3d0abbd0f7f2164b5ff1203f1179571628e715249676413ab` |
| remark-parse | `32a0017bbc88185a254352d27270d69b964e4658c2007a805552ae6af7c188de` | `6ba50d722cb259b8f2f98df1744e1bc9bad79da45e47cf8f4e5e04dae3b94cbc` | `f5d5e59e90b42453c81795faf33ac023ccf552d713024d04bcc1328e37f38a59` |
| remark | `e53800e9b7f67ff8f2d2ea822ac68f49958e35a6d5197114aa6800bc6e82f904` | `862c924b2c89d25d5627802f5b0b4daf0b3d22d2c851c9a1ea8d2f3f495b3977` | `f5357d469b8b29fd897045b9a7001e74045dcc69459df59ba098bd30c61ed155` |
| unified | `3aa46541f7491ee8c1c1376e0b42c5cbdadf5bfe68ce2b35493ede623f2a186b` | `9c3066bea62f0a82a80d0cef9bb3f424c113956479374f52af14812525390e73` | `96e24760527dc039a0bc99eebafdfdf414d52727d64c9a1229dbdd4e2ebb6ca0` |
| mdast-util-to-hast | `e7b618c8f6aca0f1b1ecddd66455e2f56eab8081ed4126bfd008456604537b54` | `d11dfb62d90351274a45aabbf4c0e8bd7137a4cf52f844e5e16148e5f8b588a6` | `4c7e25600e27d9edc8dfccbd807bab2f66ffa6e4153d7e2d5229e83a9c41fe34` |
| hast-util-to-html | `6b2f81d4f3d11537eff8afe2d7250bccfb10cef33944365b8104014eb4cf4a7e` | `1a4376d31c0bba62ef7386b78e9300b81be38fbae8580fc2bfa4eba926ae629d` | `a9a9f741bda87818f56d20e21fb5006be072bc31628bc5afbc0bda118b720092` |
| remark-rehype | `d7cb4e3e4f6eb506f93739bb485d139981a0a39526a71646dae695c671ca0971` | `ce1801b9a911e81dcb6377b52c3e128b30fea6cfc75a382c466e080ed6d754cb` | `2f82d1af65247986c25cf2be284f8f8804ad9ff7a13045c182eec08bfa9cc486` |
| rehype | `459895e2acccf3d3a639fb1de6863672aa841c561d948c44f2ff2f8bc73fccbb` | `b72a9e411eeec0a8db6d99da4c64e3e36df503bbf271920adbaa385012d8ff6c` | `961fd421036db59d3998007b194c4676fcaf4cd5be7d046d7303c6072f7b80a7` |
| rehype-stringify | `51c775f9c005caa3f489c5da36577c8baa36cc0879b984fd4a82872460bc5e34` | `a0a742d1f47252733411a55a52f95da86d05615ff91f7b84144a047d5de15f73` | `5b41b8a2bafe19a6ab3a8127aeef1ddfe837d70e303e0959b609a1b5298fa9fb` |
| remark-gfm | `4d6d19e6cea8223fc611dde254670b92167c96d2201a104d3dcd7b32d8cde267` | `331ce7120dfc27733e01e1e93429a0b5802be2585872ac8c3b4dc5764c4af84f` | `868b3d13d91ff454e4d72b1969a337b52f9c20e0e169ff8a39f12aa0a0bad05e` |
| remark-breaks | `cc56b30e62d9fb7bf5f730e02ddb114e734f003a0e3766e3abed8849a91b4689` | `8e00019479fb6097007c45889d4e31f04228dfbfa8e980cd9aae1d330af9993a` | `63e6a40d345805073826d0f80ce7baa2a6fa15bfca2eff06d814999c05789757` |
| remark-math | `f80c4832481097f98289d5852b5fdff3a0b55f15d49dfb6d8e6397acf023d4c2` | `406aa2970d7aa3e0b47d851eabc2fe3dd16ee148a60a62fcc3bea433918abf9c` | `c0c3759b4728daf9e803da6ab7255c7ad25124ee89382bd22d7f56ac51bd383e` |
| katex | `5ac9e8b8a501d45ed89f63a94cbe293e4b4c4c82a82d33d6fef29690cae61bee` | `b2e45ad36d12450fd8ade35113f7530a30cde509ee36e97406bcd8e70ee32e12` | `c741830d69da55ee94bc8cff4f3a91b1327cd02c9320142d73ccda365a02527c` |
| rehype-katex | `bd5d7d00623a856f50621bff26b075636e7bf2384c33a16b9145168129ff0604` | `d9c412e11aa975244177f3912fa05111bf63b489142872fe571c19c777e1c7f8` | `a794ab0f9dab939fd537cc6c9e2b759aa6739e500b19c567ca73cfba8ad16052` |
| react-markdown | `e0f94625f6ebcea7b8e8e0bbe677fe09d84e82e2b20a79e9aa8a89dbde756812` | `4b2c7c899fe83f7b6c2c3ebe07808f809041351401d87e29c78a7f76b6027dc7` | `8367bcbcb78bd9c77dee73bb0ed60619fcde212e044be4058baf9b9cccafe9db` |

## Baseline discrepancies

| Port | Lane | Historical raw/gzip/Brotli | Canonical raw/gzip/Brotli | Explanation |
|---|---|---|---|---|
| micromark | `official-terser` | 81191/26278/22696 | 81530/26383/22776 | The historical row did not pin its Terser version and full options; the canonical row uses Terser 5.51.2 with module=true, compress=true, and mangle=true. |
| mdast-util-from-markdown | `official-terser` | 84154/26849/23151 | 84681/27038/23279 | The historical row did not pin its Terser version and full options; the canonical row uses Terser 5.51.2 with module=true, compress=true, and mangle=true. |
| remark-parse | `official-terser` | 84339/26914/23171 | 84866/27101/23283 | The historical row did not pin its Terser version and full options; the canonical row uses Terser 5.51.2 with module=true, compress=true, and mangle=true. |
| remark | `official-terser` | 118936/37029/32279 | 119872/37366/32551 | The historical row did not pin its Terser version and full options; the canonical row uses Terser 5.51.2 with module=true, compress=true, and mangle=true. |
| remark | `lil-graph` | 205148/53914/45846 | 205300/53926/45880 | The historical row used an earlier full-graph build without an artifact hash; the canonical graph is freshly bundled from the current standard ESM and locked runtime dependencies. |
| unified | `official-graph` | 55620/11945/10434 | 55620/11900/10434 | Raw and Brotli match; only gzip differs. The historical row has no compressor artifact hash, while the canonical value uses the recorded stock zlib 1.3.1 codec binary. |
| unified | `official-terser` | 13579/4883/4425 | 13579/4862/4425 | Raw and Brotli match; only gzip differs. The historical row has no compressor artifact hash, while the canonical value uses the recorded stock zlib 1.3.1 codec binary. |
| unified | `lil-graph` | 21276/6017/5398 | 21316/6028/5409 | The historical row used an earlier full-graph build without an artifact hash; the canonical graph is freshly bundled from the current standard ESM and locked runtime dependencies. |
| mdast-util-to-hast | `official-graph` | 39281/8715/7797 | 39728/8849/7908 | The historical row is the retained site/official.js artifact (SHA-256 43d25e896e0774d87331b13c4909ea4242dc6d47347f9c294462546a3eee07db); the canonical graph is freshly resolved from the harness lock, so a difference identifies graph-input drift. |
| mdast-util-to-hast | `official-terser` | 16715/5388/4862 | 17117/5537/5016 | The historical row did not pin its Terser version and full options; the canonical row uses Terser 5.51.2 with module=true, compress=true, and mangle=true. |
| mdast-util-to-hast | `lil-graph` | 14620/4767/4290 | 14620/4762/4290 | Raw and Brotli match; only gzip differs. The historical row has no compressor artifact hash, while the canonical value uses the recorded stock zlib 1.3.1 codec binary. |
| hast-util-to-html | `official-terser` | 31835/11221/9833 | 31882/11235/9839 | The historical row did not pin its Terser version and full options; the canonical row uses Terser 5.51.2 with module=true, compress=true, and mangle=true. |
| hast-util-to-html | `lil-graph` | 30253/10067/8811 | 30253/9962/8811 | Raw and Brotli match; only gzip differs. The historical row has no compressor artifact hash, while the canonical value uses the recorded stock zlib 1.3.1 codec binary. |
| remark-rehype | `official-graph` | 39783/8849/7919 | 40230/8984/8028 | The historical row is the retained site/official.js artifact (SHA-256 3813e736e9febb3c24f541f7921b8339504272abf8d4432f16d96c022e477feb); the canonical graph is freshly resolved from the harness lock, so a difference identifies graph-input drift. |
| remark-rehype | `official-terser` | 16861/5445/4910 | 17263/5595/5061 | The historical row did not pin its Terser version and full options; the canonical row uses Terser 5.51.2 with module=true, compress=true, and mangle=true. |
| remark-rehype | `lil-graph` | 15027/4922/4390 | 15027/4911/4390 | Raw and Brotli match; only gzip differs. The historical row has no compressor artifact hash, while the canonical value uses the recorded stock zlib 1.3.1 codec binary. |
| rehype | `official-graph` | 415899/90181/73110 | 438508/91765/74480 | The historical row is the retained site/official.js artifact (SHA-256 b009e9c76d140d6054dbd1574bc00bb5f7877cb552dbce335cf76ea488607dc4); the canonical graph is freshly resolved from the harness lock, so a difference identifies graph-input drift. |
| rehype | `official-terser` | 250978/72154/57948 | 221625/67746/55080 | The historical raw and Brotli values reproduce by minifying the retained historical graph with module=false; the canonical lane uses a fresh graph and the required module=true. |
| rehype | `lil-graph` | 372587/79671/64992 | 372587/78982/64992 | Raw and Brotli match; only gzip differs. The historical row has no compressor artifact hash, while the canonical value uses the recorded stock zlib 1.3.1 codec binary. |
| rehype-stringify | `official-terser` | 31928/11255/9875 | 31975/11269/9886 | The historical row did not pin its Terser version and full options; the canonical row uses Terser 5.51.2 with module=true, compress=true, and mangle=true. |
| rehype-stringify | `lil-graph` | 30572/10423/9141 | 30572/10302/9141 | Raw and Brotli match; only gzip differs. The historical row has no compressor artifact hash, while the canonical value uses the recorded stock zlib 1.3.1 codec binary. |
| remark-gfm | `official-terser` | 42094/12387/11149 | 42343/12446/11238 | The historical row did not pin its Terser version and full options; the canonical row uses Terser 5.51.2 with module=true, compress=true, and mangle=true. |
| remark-gfm | `lil-graph` | 38873/13194/11617 | 38873/13144/11617 | Raw and Brotli match; only gzip differs. The historical row has no compressor artifact hash, while the canonical value uses the recorded stock zlib 1.3.1 codec binary. |
| remark-breaks | `official-terser` | 2988/1284/1173 | 3045/1299/1198 | The historical row did not pin its Terser version and full options; the canonical row uses Terser 5.51.2 with module=true, compress=true, and mangle=true. |
| remark-breaks | `lil-graph` | 2746/1261/1131 | 2746/1258/1131 | Raw and Brotli match; only gzip differs. The historical row has no compressor artifact hash, while the canonical value uses the recorded stock zlib 1.3.1 codec binary. |
| remark-math | `official-terser` | 6350/2324/2097 | 6442/2378/2150 | The historical row did not pin its Terser version and full options; the canonical row uses Terser 5.51.2 with module=true, compress=true, and mangle=true. |
| remark-math | `lil-graph` | 7615/2885/2600 | 7615/2887/2600 | Raw and Brotli match; only gzip differs. The historical row has no compressor artifact hash, while the canonical value uses the recorded stock zlib 1.3.1 codec binary. |
| katex | `official-terser` | 267050/76480/63044 | 267745/76650/63137 | The historical row did not pin its Terser version and full options; the canonical row uses Terser 5.51.2 with module=true, compress=true, and mangle=true. |
| katex | `lil-graph` | 295886/84675/69669 | 295886/83961/69669 | Raw and Brotli match; only gzip differs. The historical row has no compressor artifact hash, while the canonical value uses the recorded stock zlib 1.3.1 codec binary. |
| rehype-katex | `official-graph` | 898046/186731/149397 | 898046/185933/149397 | Raw and Brotli match; only gzip differs. The historical row has no compressor artifact hash, while the canonical value uses the recorded stock zlib 1.3.1 codec binary. |
| rehype-katex | `official-terser` | 474237/139471/113063 | 474237/138812/113063 | Raw and Brotli match; only gzip differs. The historical row has no compressor artifact hash, while the canonical value uses the recorded stock zlib 1.3.1 codec binary. |
| rehype-katex | `lil-graph` | 586338/104976/84501 | 586338/104850/84501 | Raw and Brotli match; only gzip differs. The historical row has no compressor artifact hash, while the canonical value uses the recorded stock zlib 1.3.1 codec binary. |
| react-markdown | `official-graph` | 294369/59392/50469 | 295252/59175/50482 | No historical graph artifact or dependency lock was retained; the canonical graph is freshly resolved from the harness lock. |
| react-markdown | `official-terser` | 117674/35024/31082 | 117759/34914/31092 | The Terser version/options match, but the historical row used its prior graph without a retained artifact hash; the canonical row minifies the freshly locked graph. |
| react-markdown | `lil-graph` | 216385/56632/47862 | 216436/56636/47861 | The historical row used an earlier full-graph build without an artifact hash; the canonical graph is freshly bundled from the current standard ESM and locked runtime dependencies. |

13 historical lanes matched all three metrics exactly; only differences are listed above.

## Test verification

- Contract tests: 11 passed, 0 failed.
- Input audit: 16 ports, 174 pinned upstream runtime modules, and all current sibling source inventories passed.
- `--run-tests` ran the exact `npm test` scripts for 13 read-only-safe ports: 5,632 passed, 0 failed. Report SHA-256: `a4184d57c2ad9a1e6a70e2654460416533c465a7e64112ae9caac651bd72128a`.
- The exact scripts for `remark-gfm`, `remark-breaks`, and `remark-math` were not run because each starts by rebuilding sibling artifacts. Their non-mutating type/test tails ran instead: 99 passed, 0 failed. Across all executed test bodies: 5,731 passed, 0 failed.

Exact read-only-safe command:

```sh
node comparison/markdown-stack/run.mjs --run-tests --only micromark,mdast-util-from-markdown,remark-parse,remark,unified,mdast-util-to-hast,hast-util-to-html,remark-rehype,rehype,rehype-stringify,katex,rehype-katex,react-markdown --json comparison/markdown-stack/.work/tests-read-only.json
```

## Disputed claims

- **micromark: approximately 13.0 KB Brotli for official Terser (diagnostic-only).** Reproduced only with esbuild's browser condition, which selects decode-named-character-reference/index.dom.js and relies on the host DOM instead of bundling character-entities. The canonical neutral library graph includes that runtime data and remains approximately 22.7 KB Brotli. Reproduced diagnostic: 52901/14562/13006 bytes (raw/gzip/Brotli), SHA-256 `90822d8b79d0641276a3cf0ca9a318343d933ca20d33755a395474caf7e16c7d`.

## Reproduction

```sh
npm --prefix comparison/markdown-stack test
node comparison/markdown-stack/run.mjs --check-inputs --json comparison/markdown-stack/.work/input-audit.json
node comparison/markdown-stack/run.mjs --measure --json comparison/markdown-stack/.work/measurements.json --markdown comparison/markdown-stack/REPORT.md
node comparison/markdown-stack/run.mjs --run-tests --json comparison/markdown-stack/.work/tests.json
```

Node: `v20.19.0`; esbuild: `0.28.1`; Terser: `5.51.2`; lock SHA-256: `6f506ce6b58b2dc94400c88f816f3df1bcc247ee7d56216ff718bcb8b9ab742c`; codec SHA-256: `c7a6cf4a12db10bcba3af51a473e870e52ce19b6587bc6e1100ead4565bb613b`.

Harness SHA-256: `run.mjs` 95dacd118fcab6e07afb7001f955d128cb69c24257ea38b71e528967482d98b0; `contract.mjs` f9c5a48ce9843c46b58dcd4731969de699ce559edcca429229fa34be1cd2ab8e; `contract.test.mjs` 736b39d6debcfd23aab29ff879e7628e8cfafd10ad39f7859e37364295a91194; `package.json` 69f0ed48f846bfedcb97a1479a535c0b80411104ef686d19f86078c2b53587a1; `package-lock.json` 6f506ce6b58b2dc94400c88f816f3df1bcc247ee7d56216ff718bcb8b9ab742c.

esbuild receives each exact official public root entry directly, which preserves every root export. Standalone standard Lil ESM files are copied byte-for-byte; only entries with runtime imports are bundled to complete their graph, and no Lil graph is post-minified. The equivalent official graph is minified once with pinned Terser options. React and `react/*` are external only for React Markdown, on both sides. All other sibling artifacts, including every closed build, are diagnostic and can never be selected.
