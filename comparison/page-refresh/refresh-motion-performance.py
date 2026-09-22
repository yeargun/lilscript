import pathlib,json,shutil,hashlib,re,sys
run=pathlib.Path(sys.argv[1]);root=pathlib.Path(sys.argv[2]);site=root/'site';bench=run/'motion-benchmark'
def read(p):return json.loads(p.read_text())
def write(p,j):p.write_text(json.dumps(j,indent=2)+'\n')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
data=read(bench/'performance.json');natural=read(bench/'natural-validation.json')
measured=[x for x in data['workloads'] if x['status']=='measured']
assert len(measured)==len(data['workloads'])==len(natural['results']) and all(x['matches'] for x in natural['results'])
assert {x['id'] for x in measured}=={x['id'] for x in natural['results']}
assert all(x['validation']['original']['backend']==x['validation']['lilscript']['backend'] for x in measured)
assert all(x['validation']['original']['libraryRaf']==x['validation']['lilscript']['libraryRaf'] for x in measured)
comparison=read(site/'comparison.json');build=read(site/'source-build.json')
for side in ['original','lilscript']:assert data['inputs'][side]['sha256']==comparison['esm'][side]['sha256']
assert natural['harnessSha256']==data['inputs']['harnessSha256']
assert natural['checkerSha256']==sha(root/'scripts/check-natural-performance.mjs')
data['machine'].update(provider='Azure',instanceClass=comparison['machine']['instanceClass'])
data['sources']={'compiler':comparison['compiler'],'port':build['portSource'],'original':build['source']}
data['protocol']['playwright']='1.58.2';data['naturalPlaybackValidation']='./natural-validation.json';write(bench/'performance.json',data)
output=site/'performance';output.mkdir(exist_ok=True)
for file in ['index.html','harness.mjs','workloads.json','performance.json','natural-validation.json']:shutil.copy2(bench/file,output/file)
shutil.copytree(bench/'inputs',output/'inputs',dirs_exist_ok=True)
summary={k:v for k,v in data.items() if k!='workloads'}
summary['workloads']=[{k:v for k,v in x.items() if k not in ['samples','validation']}|{'validation':{'errors':x['validation']['errors'],'backendAgreement':x['validation']['original']['backend']==x['validation']['lilscript']['backend'],'libraryRafAgreement':x['validation']['original']['libraryRaf']==x['validation']['lilscript']['libraryRaf'],'activeNativeProperties':x['validation']['original']['backend']['activeNativeProperties'],'libraryRaf':{lane:x['validation'][lane]['libraryRaf'] for lane in ['original','lilscript']}}} for x in data['workloads']]
write(site/'performance.json',summary)
rows=[]
for work in summary['workloads']:
 if work['status']!='measured':
  rows.append(f'<tr data-workload="{work["id"]}"><th scope="row">{work["label"]}</th><td colspan="7">Behavior mismatch: string transforms do not follow the requested timeline. No speed score.</td></tr>');continue
 s=work['summary'];o=s['original'];l=s['lilscript'];interval='–'.join(f'{v:.3f}' for v in s['cpuRatio95']);verdict='Within ±5%' if s['withinFivePercent'] else ('Lower CPU' if s['cpuRatio95'][1]<1 else 'Higher CPU' if s['cpuRatio95'][0]>1 else 'Inconclusive')
 rows.append(f'''<tr data-workload="{work['id']}"><th scope="row">{work['label']}</th><td>{o['mainThreadMs']:.2f}</td><td>{l['mainThreadMs']:.2f}</td><td><strong>{s['cpuRatio']:.3f}×</strong><br>{interval}<br>{verdict}</td><td>{o['scriptMs']:.2f} / {l['scriptMs']:.2f}</td><td>{o['styleLayoutMs']:.2f} / {l['styleLayoutMs']:.2f}</td><td>{o['setupMs']:.2f} / {l['setupMs']:.2f}</td><td>{o['frameP95Ms']:.2f} / {l['frameP95Ms']:.2f}</td></tr>''')
p=site/'index.html';html=p.read_text()
match=re.search(r'<!-- performance:start -->[\s\S]*?<!-- performance:end -->',html);assert match
block=re.sub(r'<tbody>[\s\S]*?</tbody>','<tbody>'+''.join(rows)+'</tbody>',match[0])
waapi=[x for x in measured if x['api']=='animateMini'];equivalent=sum(x['summary']['withinFivePercent'] for x in waapi)
m=data['machine']
notes=f"""<strong>What the numbers mean.</strong> CPU is Chromium renderer main-thread task time during an 800 ms observation window; animations last 600 ms. Numbers are medians; ratios use paired medians with a 95% bootstrap interval. Equivalence requires the entire interval inside 0.95–1.05. {equivalent} of {len(waapi)} mini WAAPI workloads meet that criterion. JavaScript, style and layout costs are reported separately. Lower CPU is not a measure of better animation behavior.<br><strong>Behavior contract.</strong> The rewrite must preserve Motion's animation behavior. All {len(measured)} workloads match native animation calls/options, active native properties, five paused timeline positions and final values. Native transform/opacity uses WAAPI; x/y and layout use JavaScript with native opacity; standalone MotionValues use JavaScript. A separate natural-playback check inspects every animated property on every element across three fresh pages per side. The repository also compares easing, repeats, pause/seek/replay/reverse/stop, interruption, callback order and library RAF under a shared clock. These checks cover the documented scenarios; low-level layout/rendering adapters remain incomplete.<br><strong>Frame activity.</strong> Frame p95 is the benchmark observer's RAF cadence. Library RAF request/execution counts match during untimed validation, with no queued callbacks after stop. Equal display cadence alone does not establish equal runtime cost.<br><strong>Machine and method.</strong> Both lanes use the exact 312-export ESM files from the headline size comparison, with the same toolchain and public extern reservations. Azure {m['instanceClass']} · {m['cpu']} · {m['logicalCpus']} vCPUs · {m['memoryBytes']/2**30:.1f} GiB RAM · {m['os']} · Node {m['node'].lstrip('v')} · headless Chromium {data['browser']} · 1280×900, DPR 1. Fresh page for every run; original/LilScript order alternates. Bundle download, module evaluation and initial DOM creation are outside the CPU interval. Headless renderer CPU does not measure all GPU/compositor work.<br><a href="./performance/performance.json">All paired samples and backend/RAF checks ↗</a> · <a href="./performance/natural-validation.json">Natural playback checks ↗</a> · <a href="https://github.com/yeargun/motionlil/blob/main/scripts/measure-performance.mjs">Reproduce the measurement ↗</a>"""
block=re.sub(r'(<div class="method-note"><p>)[\s\S]*?(</p></div>)',lambda m:m[1]+notes+m[2],block)
p.write_text(html[:match.start()]+block+html[match.end():])
comparison['compatibility']=f"All {len(measured)} browser workloads match native backends and sampled timelines; recorded runtime checks cover easing, repeats, interpolation, lifecycle callbacks, interruption and frame scheduling. Low-level layout/rendering adapters remain incomplete; matching all original export names does not establish complete API parity."
comparison['performance']={'summary':'./performance.json','raw':'./performance/performance.json','naturalValidation':'./performance/natural-validation.json','repetitions':data['protocol']['repetitions'],'matchingWorkloads':len(measured),'mismatchingWorkloads':len(data['workloads'])-len(measured),'waapiWithinFivePercent':equivalent==len(waapi)}
write(site/'comparison.json',comparison)
print('Prepared current Motion performance table with unchanged markup and CSS')
