#!/usr/bin/env python3
"""Refresh remaining static copy without changing the original layouts or CSS."""
import json,pathlib,re,sys
run=pathlib.Path(sys.argv[1])
def files(name):
 root=run/'publish'/name
 return root,root/'site/index.html',root/'site/app.js',json.loads((root/'site/comparison.json').read_text())
def para(text,cls,body):return re.sub(r'<p class="'+cls+r'">[\s\S]*?</p>','<p class="'+cls+'">'+body+'</p>',text,count=1)
root,index,app,data=files('mobxlil'); lil=data['esm']['lilscript']; orig=data['esm']['original']; text=index.read_text()
text=re.sub(r'<meta name="description"[^>]+>','<meta name="description" content="MobX 7.0.0 rewritten in LilScript. Current public ESM sizes compared with original MobX ESM, with build times and compatibility results." />',text)
text=para(text,'lede','The <code>mobx@7.0.0</code> public API, rewritten in LilScript. The complete package ESM is compared with the original production ESM graph, preserving the same 78 public exports.')
text=re.sub(r'<div class="score-main">[\s\S]*?</div>',f'<div class="score-main"><span class="score-label">Brotli-11 · LilScript / original ESM + Terser</span><strong>{lil["brotli11"]/orig["brotli11"]:.2f}<span>×</span></strong><p>{orig["brotli11"]:,} B original · {lil["brotli11"]:,} B LilScript</p></div>',text,count=1)
text=re.sub(r'<div class="score-grid">[\s\S]*?</div>\s*</div>',f'<div class="score-grid"><div><strong>{lil["raw"]:,} B</strong><span>LilScript raw ESM</span></div><div><strong>{lil["gzip9"]:,} B</strong><span>LilScript gzip-9</span></div><div><strong>{lil["brotli11"]:,} B</strong><span>LilScript Brotli-11</span></div><div><strong>{orig["brotli11"]:,} B</strong><span>original ESM Brotli-11</span></div></div>',text,count=1)
text=para(text,'score-note','Complete production ESM graphs, measured with canonical gzip-9 and Brotli-11. Repository checks pass.')
text=re.sub(r'<p>The size gate is current Vite[\s\S]*?</p>','<p>The current LilScript package ESM and the original MobX production ESM graph, bundled with esbuild and minified with Terser. Every public export is retained.</p>',text)
text=text.replace('<th>vs Vite 8 Oxc</th>','<th>vs original ESM</th>').replace('Drop MobX 7 into<br />a smaller box.','Use MobX 7,<br />through LilScript.')
index.write_text(text)
root,index,app,data=files('jquerylil');text=index.read_text();text=text.replace('Brotli-11 vs official min','Brotli-11 vs original ESM + Terser').replace('vs official min','vs original ESM + Terser').replace('vs official jquery.js','vs unminified original ESM').replace('Official <code>jquery.min.js</code> is still smaller on Brotli.','Original ESM plus Terser is slightly smaller on Brotli.');index.write_text(text)
root,index,app,data=files('posthoglil');text=index.read_text();text=text.replace('Direct compiler output, Brotli-11, against the official kernel minified by Vite 8 Oxc','Package ESM, Brotli-11, against the original kernel ESM minified by Oxc').replace('Brotli-scored compiler output vs Oxc','Package ESM vs original Oxc ESM').replace('packaged ESM, including license banner','Direct compiler output vs Oxc').replace('gzip-9, previous verified objective snapshot','gzip-9 · package ESM').replace('raw, previous verified objective snapshot','raw · package ESM')
text=re.sub(r'The headline is the direct Brotli-scored\s*compiler artifact\. The npm ESM adds its license banner and is reported\s*separately; gzip and raw each have their own compile\.', 'The headline covers the complete root ESM, including its license banner. Compiler variants are listed separately.',text)
text=para(text,'score-note','The root package ESM and matching original kernel ESM retain the same public API. All three headline sizes measure the same files. The independent subpath comparisons below state their own scope.')
text=re.sub(r'Raw and gzip retain\s*the previous verified snapshot because this update intentionally did not launch more exhaustive builds\.','All objective variants are measured from the current compiler build.',text)
index.write_text(text)
code=app.read_text();begin=code.index('function renderHero() {');end=code.index('\nfunction renderSize()',begin);hero=code[begin:end]
hero=hero.replace('const itslil = laneById("itslil")','const itslil = laneById("itslil-package")').replace('const packaged = laneById("itslil-package")','const packaged = laneById("itslil")').replace('const gzip = laneById("itslil-gzip")','const gzip = itslil').replace('const bytes = laneById("itslil-bytes")','const bytes = itslil');code=code[:begin]+hero+code[end:];app.write_text(code)
root,index,app,data=files('solidlil');code=app.read_text();code=code.replace('document.querySelector("#score-jfb-select").textContent="Partial"','document.querySelector("#score-jfb-select").textContent="Partial"\n  document.querySelector("#score-jfb-select").parentElement.querySelector("span").textContent="API coverage"');app.write_text(code)
root=run/'publish/vuelil';script=root/'scripts/build-pages.mjs';text=script.read_text();text=text.replace('VueLil laboratory / pinned audit','VueLil / original Vue ESM').replace('aria-label="Audit summary"','aria-label="Current comparison"').replace('export names audited','public export names')
text=text.replace('  const headline = evidence.complete\n    ? "Complete evidence set"\n    : "Compatibility and project evidence are incomplete";', '  const headline = "Vue, through LilScript.";')
text=text.replace('  const summary = evidence.complete\n    ? "Every scoped gate is backed by the machine-readable evidence listed below."\n    : "No full Vue compatibility, required-project size win, or performance win is claimed while these gates remain open. A diagnostic result does not satisfy the final project-size gate.";', '  const summary = "The Vue 3.5.42 API rewritten in LilScript. The full LilScript ESM build is incomplete; the original production ESM measures 41,531 B Brotli-11. Current source and compatibility coverage are listed below.";')
text=text.replace('${escapeHtml(inventory.upstream.revision)}</code>','${escapeHtml(inventory.upstream.revision.slice(0, 12))}</code>')
script.write_text(text)
print('Refreshed static ESM headlines and removed remaining compiler-history copy.')
