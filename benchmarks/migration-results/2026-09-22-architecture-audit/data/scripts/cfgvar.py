#!/usr/bin/env python3
# cfgvar.py <in.toml> <out.toml> key=value ... : set keys in [javascript] (or section.key=value)
import sys, re
src, dst, *pairs = sys.argv[1:]
lines = open(src).read().split('\n')
sets = {}
for p in pairs:
    k, v = p.split('=', 1)
    sec, key = (k.split('.', 1) if '.' in k else ('javascript', k))
    sets.setdefault(sec, {})[key] = v
out, cur, done = [], None, set()
def flush(sec):
    for key, v in sets.get(sec, {}).items():
        if (sec, key) not in done:
            out.append(f'{key} = {v}'); done.add((sec, key))
for ln in lines:
    m = re.match(r'^\s*\[([^\]]+)\]\s*$', ln)
    if m:
        if cur is not None: flush(cur)
        cur = m.group(1).strip(); out.append(ln); continue
    km = re.match(r'^\s*([A-Za-z0-9_\-]+)\s*=', ln)
    if km and cur in sets and km.group(1) in sets[cur]:
        out.append(f'{km.group(1)} = {sets[cur][km.group(1)]}'); done.add((cur, km.group(1))); continue
    out.append(ln)
if cur is not None: flush(cur)
for sec, kv in sets.items():
    missing = [k for k in kv if (sec, k) not in done]
    if missing:
        out.append(f'[{sec}]')
        for k in missing: out.append(f'{k} = {kv[k]}')
open(dst, 'w').write('\n'.join(out))
