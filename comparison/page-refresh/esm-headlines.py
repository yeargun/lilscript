#!/usr/bin/env python3
"""Put complete current ESM entries into the sites' existing scoreboards."""
import json
import pathlib
import re
import sys

run=pathlib.Path(sys.argv[1])
def load(name):
    root=run/'publish'/name
    return root,root/'site/index.html',root/'site/app.js',json.loads((root/'site/comparison.json').read_text())
def esm_import(code):
    if 'const esmComparison =' not in code:code='const esmComparison = await fetch("./comparison.json").then(response => response.json())\n'+code
    return code
def paragraph(text,css,value):return re.sub(r'<p class="'+css+r'">[\s\S]*?</p>','<p class="'+css+'">'+value+'</p>',text,count=1)

root,index,app,data=load('motionlil')
lil=data['esm']['lilscript'];original=data['esm']['original'];ratio=lil['brotli11']/original['brotli11']
text=index.read_text()
text=re.sub(r'<div class="score-main">[\s\S]*?</div>',f'''<div class="score-main">
            <span class="score-label">Reusable ESM · LilScript / original</span>
            <strong>{ratio:.2f}<span>× Brotli</span></strong>
            <p>{lil['brotli11']:,} B vs {original['brotli11']:,} B</p>
          </div>''',text,count=1)
text=re.sub(r'<div class="score-grid">[\s\S]*?</div>\s*</div>',f'''<div class="score-grid">
            <div><strong>{lil['raw']:,} B</strong><span>LilScript raw ESM</span></div>
            <div><strong>{lil['gzip9']:,} B</strong><span>LilScript gzip-9</span></div>
            <div><strong>{lil['brotli11']:,} B</strong><span>LilScript Brotli-11</span></div>
            <div><strong>{original['brotli11']:,} B</strong><span>Original ESM Brotli-11</span></div>
          </div>''',text,count=1)
text=paragraph(text,'score-note','Complete ESM entry graphs with all public exports retained. Browser compatibility checks are incomplete.')
text=text.replace('Recovered from the LilScript browser suite','Motion DOM examples')
text=re.sub(r'<p>The gate metric is Brotli 11[\s\S]*?</p>','<p>The complete reusable LilScript ESM entry compared with the original Motion ESM graph. Raw, gzip-9 and Brotli-11 sizes use the same public-entry scope.</p>',text)
text=text.replace('<th>Case</th>','<th>ESM metric</th>').replace('<th>Saved</th>','<th>Difference</th>')
text=re.sub(r'(<div class="method-note">\s*)<p>[\s\S]*?</p>',r'\1<p><strong>Public ESM.</strong> These figures cover the reusable package entry and its dependencies. Demonstrations illustrate the API. The ESM size includes all entry-point exports.</p>',text)
text=re.sub(r'(<div class="bar-motion"[^>]*><span>Motion</span><strong>)[^<]*',lambda m:m[1]+f"{original['brotli11']:,} B",text)
text=re.sub(r'(<div class="bar-lil"[^>]*><span>LilScript</span><strong>)[^<]*',lambda m:m[1]+f"{lil['brotli11']:,} B",text)
maximum=max(lil['brotli11'],original['brotli11'])
text=text.replace('<div class="bar-motion">',f'<div class="bar-motion" style="width:{100*original["brotli11"]/maximum:.3f}%">').replace('<div class="bar-lil">',f'<div class="bar-lil" style="width:{100*lil["brotli11"]/maximum:.3f}%">')
index.write_text(text)
code=esm_import(app.read_text())
code=code.replace('−${example.reduction.toFixed(1)}%','${example.group}').replace('${formatter.format(example.lilscript)} B Brotli','Motion DOM')
code=re.sub(r'function renderResults\(\) \{[\s\S]*?\n\}', '''function renderResults() {
  const {lilscript, original}=esmComparison.esm
  resultsBody.innerHTML = [["Raw", "raw"], ["gzip-9", "gzip9"], ["Brotli-11", "brotli11"]].map(([label,key]) => {
    const difference=(lilscript[key]/original[key]-1)*100
    return `<tr><th scope="row">${label}</th><td>${formatter.format(original[key])} B</td><td>${formatter.format(lilscript[key])} B</td><td>${(lilscript[key]/original[key]).toFixed(3)}×</td><td><strong>${Math.abs(difference).toFixed(1)}% ${difference>0?"larger":"smaller"}</strong></td></tr>`
  }).join("")
}''',code)
app.write_text(code)

root,index,app,data=load('monacolil')
text=index.read_text().replace('by submodule.','compiled.')
text=paragraph(text,'lede','An independent LilScript implementation of <code>monaco-editor@0.56.0</code>. The headline compares the current public ESM entries, with all exported code retained. The LilScript implementation covers part of Monaco’s API; full feature parity is not established. The source catalog below shows the ported modules.')
text=re.sub(r'<span class="score-label">[^<]*</span>','<span class="score-label">Public ESM · partial implementation · Lil / original</span>',text,count=1)
text=text.replace('<span>median Lil / Oxc</span>','<span>original ESM Brotli</span>').replace('<strong>Vite 8</strong><span>Oxc minify (headline)</span>','<strong>ESM</strong><span>complete entry graphs</span>')
text=paragraph(text,'score-note','Current ESM output compared with the original ESM graph. The API coverage differs, so the byte ratio does not establish a feature-equivalent size win.')
text=text.replace('Diagnostic · do not headline','Public ESM comparison').replace('Whole IDE<br />bytes.','Current ESM<br />bytes.')
text=re.sub(r'<p>Production <code>ide.js</code>[\s\S]*?</p>','<p>The current public ESM entry and original Monaco ESM graph, measured as JavaScript. CSS and workers are separate assets. The LilScript implementation has partial API coverage.</p>',text)
index.write_text(text)
code=esm_import(app.read_text())
code=re.sub(r'function renderHero\(\) \{[\s\S]*?\n\}', '''function renderHero() {
  const {lilscript,original}=esmComparison.esm
  document.querySelector("#hero-ratio").innerHTML=`${(lilscript.brotli11/original.brotli11).toFixed(2)}<span>×</span>`
  document.querySelector("#hero-bytes").textContent=`${formatter.format(lilscript.brotli11)} B LilScript / ${formatter.format(original.brotli11)} B original`
  document.querySelector("#hero-modules").textContent=String(data.catalog.ported)
  document.querySelector("#hero-median").textContent=`${formatter.format(original.brotli11)} B`
}''',code)
code=re.sub(r'function renderProduction\(\) \{[\s\S]*?\n\}', '''function renderProduction() {
  const {lilscript,original}=esmComparison.esm
  document.querySelector("#production-body").innerHTML=`<tr><th scope="row">Public ESM · partial LilScript implementation</th><td>${bytes(original.raw)}</td><td>${bytes(original.brotli11)}</td><td>${bytes(lilscript.raw)}</td><td>${bytes(lilscript.brotli11)}</td><td><strong>${times(lilscript.brotli11/original.brotli11)}</strong></td></tr>`
  const max=Math.max(original.brotli11,lilscript.brotli11)
  document.querySelector("#total-bar").innerHTML=[["Original ESM",original,"bar-official"],["LilScript ESM",lilscript,"bar-lil"]].map(([label,lane,cls])=>`<div class="${cls}" style="width:${Math.max(18,lane.brotli11/max*100)}%"><span>${label}</span><strong>${bytes(lane.brotli11)} B</strong></div>`).join("")
}''',code)
app.write_text(code)

root,index,app,data=load('solidlil')
text=index.read_text().replace('Exact-comparison gate','Public ESM · partial implementation')
text=text.replace('<span>current CPU</span>','<span>original ESM Brotli</span>').replace('<span>current select</span>','<span>API comparison</span>')
index.write_text(text)
code=esm_import(app.read_text())
if 'function renderCurrentEsm()' not in code:
    code+='''
function renderCurrentEsm() {
  const {lilscript,original}=esmComparison.esm
  document.querySelector("#score-jfb-main").textContent=`${formatter.format(lilscript.brotli11)} B`
  document.querySelector("#score-jfb-bytes").textContent=`LilScript ESM · ${formatter.format(original.brotli11)} B original ESM`
  document.querySelector("#score-jfb-gzip").textContent=`${formatter.format(lilscript.gzip9)} B`
  document.querySelector("#score-jfb-raw").textContent=`${formatter.format(lilscript.raw)} B`
  document.querySelector("#score-jfb-cpu").textContent=`${formatter.format(original.brotli11)} B`
  document.querySelector("#score-jfb-select").textContent="Partial"
  const max=Math.max(lilscript.brotli11,original.brotli11)
  document.querySelector("#total-bar").innerHTML=[["Original ESM",original,"bar-solid"],["LilScript ESM",lilscript,"bar-lil"]].map(([name,lane,cls])=>`<div class="${cls}" style="width:${Math.max(18,lane.brotli11/max*100)}%"><span>${name}</span><strong>${formatter.format(lane.brotli11)} B</strong></div>`).join("")
  resultsBody.innerHTML=`<tr><th scope="row">Public ESM · partial implementation</th><td>${formatter.format(original.raw)}</td><td>${formatter.format(lilscript.raw)}</td><td>${formatter.format(original.gzip9)}</td><td>${formatter.format(lilscript.gzip9)}</td><td>${formatter.format(original.brotli11)}</td><td>${formatter.format(lilscript.brotli11)}</td><td>API coverage differs</td></tr>`
}
renderCurrentEsm()
'''
app.write_text(code)

root,index,app,data=load('zodlil')
text=index.read_text()
text=paragraph(text,'lede','The <code>zod@4.4.3</code> API reimplemented in LilScript. Size comparisons include the full public ESM graph: the compiled core, compatibility layer, asynchronous API, JSON Schema helpers and locales. The original ESM is measured through Oxc and Terser with the same public exports retained.')
text=paragraph(text,'score-note','Complete package ESM sizes, measured with canonical gzip-9 and Brotli-11. Compatibility checks: 973/1,317 pass.')
text=text.replace('LilScript is not post-minified. Closer-world mangles internals; normal does not.\n              The public API stays named in both. Ratios are lane / official Oxc closer-world.','The full LilScript package graph is bundled as ESM with esbuild. Public export names are retained. Ratios are ESM bytes / original Oxc ESM bytes.')
text=text.replace('The wire number. LilScript closer-world is the published core.','The wire number for the complete reusable ESM graph.')
text=re.sub(r'(<div class="method-note">\s*)<p>[\s\S]*?</p>',r'\1<p><strong>Package ESM.</strong> Both sides retain their public exports. The LilScript graph includes its core, compatibility layer, JSON Schema, asynchronous API and locale dependency. The original rows compare Oxc and Terser minification of the same upstream ESM.</p>',text)
index.write_text(text)
test=root/'test/site.test.mjs';text=test.read_text().replace('compares closer-world and normal LilScript lanes against official minify','compares the complete LilScript ESM graph against original ESM')
text=text.replace('    assert.match(html, /closer-world/)\n','').replace('    assert.match(html, /mangle off/)\n','')
test.write_text(text)

root=run/'publish/vuelil'
script=root/'scripts/build-pages.mjs';text=script.read_text();text=re.sub(r'^.*Before → After size migration plan.*\n','',text,flags=re.M);script.write_text(text)
print('Updated current ESM scoreboards in Motion, Monaco, Solid and Zod; removed Vue migration-history link.')
