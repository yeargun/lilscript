#!/usr/bin/env python3
"""Phase 6 census by origin: for every fold that still rewrites, how many of the texts it
rewrites are emitter renderings and how many are text-derived variants (the compiler's
string-surgery candidates: declaration respelling, pooling, cleanup variants). A fold at
zero on emitter renderings is phase 7's residue, not the emitter's.

  migration/tools/fold-origin.py <lilscript> <config.toml> <file.lil> [file.lil ...]

Prints one line per (fold, file): `<fold> <emitted activations> <derived activations> <file>`.
Needs LILSCRIPT_PEEPHOLE_TRACE (input snapshots) and LILSCRIPT_STATEMENT_TRACE ([emission]).
"""
import sys,subprocess,os
lil,cfg=sys.argv[1],sys.argv[2]
env=dict(os.environ,LILSCRIPT_PEEPHOLE_TRACE='1',LILSCRIPT_STATEMENT_TRACE='1',RAYON_NUM_THREADS='1')
for src in sys.argv[3:]:
    r=subprocess.run([lil,'--config',cfg,src,'--target','js','-o','/tmp/claude-1000/fo-out.js'],env=env,capture_output=True,text=True)
    lines=r.stderr.split('\n'); emitted=set(); runs=[]; i=0
    while i<len(lines):
        l=lines[i]
        if l=='[emission]': emitted.add(lines[i+1]); i+=2
        elif l.startswith('[peephole] input'): runs.append([lines[i+1],{}]); i+=2
        elif l.startswith('[peephole] '):
            name=l[len('[peephole] '):].split('::')[-1]
            if runs: runs[-1][1][name]=runs[-1][1].get(name,0)+1
            i+=2
        else: i+=1
    counts={}
    for text,folds in runs:
        origin=0 if text in emitted else 1
        for name,n in folds.items():
            c=counts.setdefault(name,[0,0]); c[origin]+=n
    base=os.path.basename(src)
    for name,(e,d) in sorted(counts.items()):
        print(f"{name} {e} {d} {base}")
