import sys,re,collections
lines=[l for l in open(sys.argv[1]) if l.startswith('[emission-options]')]
print("emissions:", len(lines))
fields=collections.defaultdict(collections.Counter)
tuples=collections.Counter()
for l in lines:
    body=l[len('[emission-options] IrJsOptions { '):].rstrip(' }\n')
    parts=re.findall(r'([a-z_0-9]+): ([^,]+(?:\{[^}]*\})?)', body)
    d=dict(parts); tuples[tuple(sorted(d.items()))]+=1
    for k,v in d.items(): fields[k][v]+=1
print("distinct tuples:", len(tuples))
print("fields that vary (values: count):")
for k,c in sorted(fields.items(), key=lambda kv: -len(kv[1])):
    if len(c)>1: print(f"  {k:36} {dict(c)}")
