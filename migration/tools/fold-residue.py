#!/usr/bin/env python3
"""What each fold still rewrites, exactly: diff consecutive LILSCRIPT_PEEPHOLE_TRACE
snapshots of ONE emission (use a search-off config) and print the changed token spans.

  migration/tools/fold-residue.py <lilscript> <config.toml> <fold,fold,...> <file.lil> [max changes]
"""
import difflib,re,sys,subprocess,os
lil,cfg,folds=sys.argv[1],sys.argv[2],sys.argv[3].split(',')
env=dict(os.environ,LILSCRIPT_PEEPHOLE_TRACE='1')
r=subprocess.run([lil,'--config',cfg,sys.argv[4],'--target','js','-o','/tmp/claude-1000/fr-out.js'],env=env,capture_output=True,text=True)
lines=r.stderr.split('\n'); steps=[]; i=0
while i<len(lines):
    if lines[i].startswith('[peephole] '): steps.append((lines[i][len('[peephole] '):].split('::')[-1], lines[i+1] if i+1<len(lines) else '')); i+=2
    else: i+=1
tok=lambda s: re.findall(r'[A-Za-z_$][\w$]*|\d+|\S', s)
emissions=sum(1 for n,_ in steps if n=='input')
print(f'(trace: {len(steps)} fold snapshots, {emissions} emissions)')
prev=None; emission=0; shown=0
for name,text in steps:
    if name=='input':
        prev=text; emission+=1; continue
    if prev is not None and name in folds:
        print(f"--- emission {emission}")
        a,b=tok(prev),tok(text); sm=difflib.SequenceMatcher(None,a,b,autojunk=False)
        ch=[(t,i1,i2,j1,j2) for t,i1,i2,j1,j2 in sm.get_opcodes() if t!='equal']
        print(f"=== {name}: {len(ch)} change(s)")
        for t,i1,i2,j1,j2 in ch[:int(sys.argv[5]) if len(sys.argv)>5 else 6]:
            lo=max(0,i1-14); print(f"   `{' '.join(a[lo:i2+8])}`  ->  `{' '.join(b[max(0,j1-14):j2+8])}`")
    prev=text
