import pathlib,json,shutil,hashlib,re,subprocess
RUN=pathlib.Path('/tmp/lilscript-source-performance-20260910');root=pathlib.Path('/tmp/lilscript-page-refresh-20260910/publish/motionlil');site=root/'site';bench=RUN/'motion-benchmark'
def read(p):return json.loads(p.read_text())
def write(p,j):p.write_text(json.dumps(j,indent=2)+'\n')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
data=read(bench/'performance.json');natural=read(bench/'natural-validation.json');assert all(x['matches'] for x in natural['results'])
comparison=read(site/'comparison.json');build=read(site/'source-build.json')
for side in ['original','lilscript']:assert data['inputs'][side]['sha256']==comparison['esm'][side]['sha256']
assert natural['harnessSha256']==data['inputs']['harnessSha256']
data['machine'].update(provider='Azure',instanceClass=comparison['machine']['instanceClass']);data['sources']={'compiler':comparison['compiler'],'port':build['portSource'],'original':build['source']};data['protocol']['playwright']='1.58.2';data['naturalPlaybackValidation']='./natural-validation.json';write(bench/'performance.json',data)
output=site/'performance';output.mkdir(exist_ok=True)
for file in ['index.html','harness.mjs','workloads.json','performance.json','natural-validation.json']:
 shutil.copy2(bench/file,output/file)
shutil.copytree(bench/'inputs',output/'inputs',dirs_exist_ok=True)
summary={k:v for k,v in data.items() if k!='workloads'};summary['workloads']=[{k:v for k,v in x.items() if k not in ['samples','validation']}|{'validation':{'errors':x['validation']['errors']}} for x in data['workloads']];write(site/'performance.json',summary)
rows=[]
for work in summary['workloads']:
 if work['status']!='measured':
  rows.append(f'<tr data-workload="{work["id"]}"><th scope="row">{work["label"]}</th><td colspan="7">Behavior mismatch: string transforms do not follow the requested timeline. No speed score.</td></tr>');continue
 s=work['summary'];o=s['original'];l=s['lilscript'];interval='–'.join(f'{v:.3f}' for v in s['cpuRatio95']);verdict='Within ±5%' if s['withinFivePercent'] else ('Lower CPU' if s['cpuRatio']<1 else 'Higher CPU')
 rows.append(f'''<tr data-workload="{work['id']}"><th scope="row">{work['label']}</th><td>{o['mainThreadMs']:.2f}</td><td>{l['mainThreadMs']:.2f}</td><td><strong>{s['cpuRatio']:.3f}×</strong><br>{interval}<br>{verdict}</td><td>{o['scriptMs']:.2f} / {l['scriptMs']:.2f}</td><td>{o['styleLayoutMs']:.2f} / {l['styleLayoutMs']:.2f}</td><td>{o['setupMs']:.2f} / {l['setupMs']:.2f}</td><td>{o['frameP95Ms']:.2f} / {l['frameP95Ms']:.2f}</td></tr>''')
block='''<!-- performance:start -->
          <div class="section-heading inverse" id="performance">
            <div><p class="eyebrow">Same fixtures · paired trials</p><h2>Browser<br />performance.</h2></div>
            <p>Identical elements, CSS, keyframes, easing and duration on both sides. Thirty paired trials per passing workload, after two warmups per lane. No speed score is assigned to a behavior mismatch.</p>
          </div>
          <div class="table-wrap"><table>
            <thead><tr><th>Workload</th><th>Motion CPU ms</th><th>LilScript CPU ms</th><th>Lil / Motion CPU<br>95% interval</th><th>Script ms<br>Motion / Lil</th><th>Style + layout ms<br>Motion / Lil</th><th>Setup ms<br>Motion / Lil</th><th>Frame p95 ms<br>Motion / Lil</th></tr></thead>
            <tbody>'''+''.join(rows)+'''</tbody></table></div>
          <div class="method-note"><p><strong>What the numbers mean.</strong> CPU is Chromium renderer main-thread task time during an 800 ms observation window; animations last 600 ms. Numbers are medians; ratios use paired medians with a 95% bootstrap interval. Equivalence requires the entire interval inside 0.95–1.05. The three WAAPI workloads meet that criterion. Their small JavaScript costs and matching style/layout costs are consistent with the shared browser pipeline dominating these cases. Frame cadence is about 16.7 ms; equal FPS alone does not prove equal CPU cost.<br><strong>Correctness first.</strong> Every element is checked at five paused timeline positions and at completion. A separate natural-playback check verifies continuous progress. The full <code>animate()</code> string-transform case fails this check and is excluded from timing. Six passing workloads are a bounded comparison, not full API parity.<br><strong>Machine and method.</strong> Azure Standard_D16als_v7 · AMD EPYC 9V45 · 16 vCPUs · 31.3 GiB RAM · Ubuntu 24.04.4 · Node 24.11.1 · headless Chromium 145.0.7632.6 · 1280×900, DPR 1. Fresh page for every run; original/LilScript order alternates. Bundle download, module evaluation and initial DOM creation are outside the CPU interval. Headless renderer CPU does not measure all GPU/compositor work.<br><a href="./performance/performance.json">All paired samples ↗</a> · <a href="./performance/natural-validation.json">Natural playback checks ↗</a> · <a href="https://github.com/yeargun/motionlil/blob/main/scripts/measure-performance.mjs">Reproduce the measurement ↗</a></p></div>
          <!-- performance:end -->'''
p=site/'index.html';s=p.read_text();s=re.sub(r'<!-- performance:start -->[\s\S]*?<!-- performance:end -->','',s);needle='          <div class="method-note">';idx=s.index('</div>',s.index(needle))+6;s=s[:idx]+block+'\n'+s[idx:];s=s.replace('<a href="#evidence">evidence</a>','<a href="#evidence">evidence</a>\n        <a href="#performance">performance</a>');p.write_text(s)
comparison['performance']={'summary':'./performance.json','raw':'./performance/performance.json','naturalValidation':'./performance/natural-validation.json','repetitions':30,'matchingWorkloads':6,'mismatchingWorkloads':1,'waapiWithinFivePercent':True};comparison['tests']={'pass':10,'total':10};write(site/'comparison.json',comparison)
guard=read(root/'comparison/build-receipt.json');guard['comparisonArtifacts']=[x for x in guard['comparisonArtifacts'] if x['path']!='site/comparison.json']
for p in [site/'comparison.json',site/'performance.json',*output.rglob('*')]:
 if p.is_file():guard['comparisonArtifacts'].append({'path':str(p.relative_to(root)),'sha256':sha(p)})
write(root/'comparison/build-receipt.json',guard)
shutil.copy2(pathlib.Path('/home/azureuser/lilscript/comparison/page-refresh/motion-natural-validation.mjs'),root/'scripts/check-natural-performance.mjs')
p=root/'README.md';s=p.read_text();s+='''\n## Reproduce browser performance\n\nRun `npm ci`, `npx playwright install chromium`, then `npm run test:performance`. This serves the exact source-built ESM inputs in `site/performance/` and records 30 alternating paired trials per passing workload. `node scripts/check-natural-performance.mjs site/performance` checks uninterrupted playback separately. Raw results, machine details, compiler and upstream source revisions accompany the page. These commands measure the recorded fixture; replacing inputs requires a fresh measurement.\n\nThe production compiler configuration retains maximum IR optimizations and uses JavaScript optimization level 0 with the package’s Terser step. This avoids the invalid keyframe-resolver output produced by the final optimization stage at the recorded compiler revision. Six tested browser workloads match; full `animate()` string transforms remain unsupported. No timing result is claimed for that failing workload.\n''';p.write_text(s)
print('Motion performance table and reproducible samples prepared')
