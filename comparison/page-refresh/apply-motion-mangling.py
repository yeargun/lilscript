"""Apply measured matched-scope artifacts to the existing Motion comparison page."""
import copy,hashlib,json,pathlib,re,shutil,sys
root=pathlib.Path(sys.argv[1]);site=root/'site';records=root/'comparison/mangling'
def read(p):return json.loads(p.read_text())
def write(p,d):p.write_text(json.dumps(d,indent=2)+'\n')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
data=read(site/'comparison.json');before=copy.deepcopy(data);source=read(site/'source-build.json');m=read(records/'results.json');v=read(records/'validation.json')
assert v['complete'] and all(x['matches'] for x in v['closed'])
assert m['source']['portCommit']==source['portSource']['commit']
assert m['source']['compiler']==data['compiler']
assert len(m['scope']['full'])==312
def measurement(scope,mode,lane):return next(x for x in m['measurements'] if (x['scope'],x['mode'],x['lane'])==(scope,mode,lane))
data['nativePackageEsm']=copy.deepcopy(data['esm'])
for lane in ['original','lilscript']:
 a=measurement('full','public-properties',lane);p=records/a['file'];assert sha(p)==a['sha256']
 shutil.copy2(p,site/'esm-comparison'/f'{lane}.js')
 data['esm'][lane]={k:a[k] for k in ['raw','gzip9','brotli11','sha256']}|{'file':f'./esm-comparison/{lane}.js'}
data['scope']='Identical 312 original Motion export names retained on both sides. Both ESM graphs use esbuild 0.28.1, Terser 5.51.2, ES2022, three compression passes and property mangling under the same public API, browser and dynamic-name extern reservations. React-specific entry points are excluded. Canonical gzip-9 and Brotli-11 compression. Export-name coverage does not establish complete behavior parity.'
data['comparisonAssembly']={'record':'./mangling.json','scope':'full','mode':'public-properties','toolchain':m['toolchain'],'source':m['source'],'externsSha256':m['externsSha256'],'machine':m['machine'],'timings':{lane:{k:measurement('full','public-properties',lane)[k] for k in ['bundleSeconds','minifySeconds']} for lane in ['original','lilscript']},'timingNote':'One additional ESM bundle/minify measurement per lane, after the native package build. These assembly times are separate from the three clean package-build samples.'}
data['apiCoverage'].update(originalExports=312,fullExports=327,defaultExports=52,comparisonExports=312,comparisonEntry='comparison/mangling/artifacts/full-public-properties-lilscript.js')
default=next(x for x in source['lilscriptArtifacts'] if x['path']=='dist/index.bundle.js')
data['defaultEntry']={k:default[k] for k in ['raw','gzip9','brotli11','sha256']}|{'file':'dist/index.bundle.js','exports':52,'scope':'Native package default entry; narrower than the 312-export main comparison. The matched 50-export consumer API is measured separately with the common toolchain.'}
data['measuredAt']=m['measuredAt'];source['comparisonAssembly']=data['comparisonAssembly']
write(site/'source-build.json',source);write(site/'comparison.json',data)
public=copy.deepcopy(m);public['validation']={'public':v['public'],'closedComparisons':len(v['closed']),'closedMatches':sum(x['matches'] for x in v['closed']),'complete':v['complete'],'browser':v['browser'],'record':'https://github.com/yeargun/motionlil/blob/main/comparison/mangling/validation.json'}
public['modes']['closedProperties']='Identical caller and library linked together; all eligible properties mangled under the same browser and dynamic-name reservations. Public library properties may be renamed because the caller is transformed in the same pass.'
write(site/'mangling.json',public)
html=(site/'index.html').read_text()
replacements={f'{before["esm"][lane][k]:,} B':f'{data["esm"][lane][k]:,} B' for lane in ['original','lilscript'] for k in ['raw','gzip9','brotli11']}
html=re.sub('|'.join(map(re.escape,sorted(replacements,key=len,reverse=True))),lambda x:replacements[x[0]],html)
oldratio=before['esm']['lilscript']['brotli11']/before['esm']['original']['brotli11'];ratio=data['esm']['lilscript']['brotli11']/data['esm']['original']['brotli11']
html=html.replace(f'{oldratio:.2f}<span>× Brotli',f'{ratio:.2f}<span>× Brotli')
html=html.replace('Full ESM · LilScript / original','312 exports · LilScript / original')
html=re.sub(r'(<p class="score-note">).*?(</p>)',r'\1Same 312 exports and public extern reservations. Both sides are property-mangled. Low-level layout/rendering adapters remain incomplete.\2',html)
html=html.replace('The full Motionlil entry compared with Motion’s original public ESM graph. Both retain every export; matching names alone does not establish behavior parity.','The same 312 original Motion exports, with identical bundling and minification settings. Matching names alone does not establish behavior parity.')
maximum=max(data['esm'][lane]['brotli11'] for lane in ['original','lilscript'])
for cls,lane in [('bar-motion','original'),('bar-lil','lilscript')]:html=re.sub(r'(<div class="'+cls+r'" style="width:)[^"]+',lambda x:x[1]+f'{100*data["esm"][lane]["brotli11"]/maximum:.3f}%',html)
note=f'<strong>Matched public ESM.</strong> Both graphs retain the same 312 original export names. The shipped full port exposes 327 names; its 15 extra names are excluded here. Public names stay usable by external callers. Low-level layout/rendering adapters remain incomplete. The native default port entry has 52 exports and measures {default["raw"]:,} B raw, {default["gzip9"]:,} B gzip-9 and {default["brotli11"]:,} B Brotli-11; its scope differs from this comparison.'
html=re.sub(r'<strong>Full ESM\.</strong>.*?</p>',lambda x:note+'</p>',html)
rows=[]
scopes=[('full','Full public ESM · 312 exports'),('consumer-api','Shared consumer API · 50 exports'),('closed-app','Closed application · identical caller'),('closed-full','Closed diagnostic · 312 roots + caller')]
for scope,label in scopes:
 for mode in ['identifiers','closed-properties' if scope.startswith('closed') else 'public-properties']:
  o=measurement(scope,mode,'original');l=measurement(scope,mode,'lilscript')
  policy='Identifiers' if mode=='identifiers' else 'Identifiers + properties'
  rows.append(f'<tr data-mangling="{scope}-{mode}"><th scope="row">{label}<br>{policy}</th><td>{o["raw"]:,}</td><td>{l["raw"]:,}</td><td>{o["brotli11"]:,}</td><td>{l["brotli11"]:,}</td><td>{l["brotli11"]/o["brotli11"]:.3f}×</td></tr>')
t=data['comparisonAssembly']['timings']
block='''<!-- mangling:start --><div class="section-heading inverse" id="mangling"><div><p class="eyebrow">Same scope · same settings</p><h2>Mangling<br />and scope.</h2></div><p>Original and LilScript use the same esbuild 0.28.1 and Terser 5.51.2 pipeline, ES2022 target and three compression passes. Sizes are bytes; ratios are LilScript / Motion.</p></div><div class="table-wrap"><table><thead><tr><th>Scope / mangling</th><th>Motion raw</th><th>LilScript raw</th><th>Motion Brotli-11</th><th>LilScript Brotli-11</th><th>Brotli ratio</th></tr></thead><tbody>'''+''.join(rows)+f'''</tbody></table></div><div class="method-note"><p><strong>Public builds.</strong> Both sides use the same extern reservations collected from public declarations, browser declarations and dynamic string names. Other properties are eligible for mangling. The identifiers row preserves remaining properties after compiler emission; LilScript's proven internal mangling remains enabled.<br><strong>Closed builds.</strong> The identical application caller is bundled and transformed with each library. All eligible properties can be renamed, while browser and dynamic protocols remain reserved. The application row removes unused API. The full diagnostic retains all 312 library roots under identical short aliases and includes the caller; it is not a distributable public ESM library.<br><strong>Validation.</strong> Each 312-export public mode passes 39 browser checks against the original with property names unchanged. All 72 closed comparisons match sampled values, native animation timing and checked library RAF activity. These scenarios do not establish complete API parity. No lane disables behavior to obtain a smaller result.<br><strong>Assembly timing.</strong> On the recorded Azure {data['machine']['instanceClass']} worker, the headline ESM bundling + minification took {t['original']['bundleSeconds']+t['original']['minifySeconds']:.3f} s for Motion and {t['lilscript']['bundleSeconds']+t['lilscript']['minifySeconds']:.3f} s for LilScript (one assembly sample each). Native source package-build times and machine details are recorded separately.<br><a href="./mangling.json">Complete measurement record ↗</a> · <a href="https://github.com/yeargun/motionlil/tree/main/comparison/mangling">Artifacts and reproduction ↗</a></p></div><!-- mangling:end -->'''
html=re.sub(r'<!-- mangling:start -->[\s\S]*?<!-- mangling:end -->','',html)
html=html.replace('<!-- performance:start -->',block+'<!-- performance:start -->')
(site/'index.html').write_text(html)
print('Applied identical 312-export comparison and eight mangling rows; CSS unchanged')
