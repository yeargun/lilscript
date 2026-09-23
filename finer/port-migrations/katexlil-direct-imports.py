# srcedit.py <srcdir> <flags: B,C,D>  -- source-level versions of lever (b),(c),(d) on a scratch katexlil src tree
import re,sys,os
root=sys.argv[1]; flags=set(sys.argv[2].split(','))
files=[]
for d,_,fs in os.walk(root):
    for f in fs:
        if f.endswith('.lil'): files.append(os.path.join(d,f))
def rd(p): return open(p).read()
def wr(p,s): open(p,'w').write(s)
def import_path(s,name):
    m=re.search(r'import \{[^}]*\b'+name+r'\b[^}]*\} from "([^"]+)";',s); return m.group(1) if m else None
def declares(s,name):  # local declaration / param with this name
    return re.search(r'\b(JsValue|number|int|bool|string|void)\s+'+name+r'\b',s) is not None
def add_import(s,names,path):
    already=set()
    for m in re.finditer(r'import \{([^}]*)\} from "'+re.escape(path)+'";',s):
        already|={x.strip().split(' as ')[-1] for x in m.group(1).split(',')}
    need=[n for n in names if n not in already]
    if not need: return s
    # insert after last import line
    last=list(re.finditer(r'^import .*;$',s,re.M))[-1]
    return s[:last.end()]+'\nimport { '+', '.join(need)+' } from "'+path+'";'+s[last.end():]
stats={}
def bump(k,n=1): stats[k]=stats.get(k,0)+n
if 'B' in flags:
    MT=['MathNode','TextNode','SpaceNode','newDocumentFragment']
    mp=os.path.join(root,'mathMLTree.lil'); ms=rd(mp)
    ms=ms.replace('export { __star, newDocumentFragment, MathNode, TextNode, mathMLTree as mathMLTree };','export { __star, newDocumentFragment, MathNode, TextNode, SpaceNode, mathMLTree as mathMLTree };'); wr(mp,ms)
    bp=os.path.join(root,'buildCommon.lil'); bs=rd(bp)
    bc_names=set()
    for p in files:
        if p in (mp,bp): continue
        s=rd(p); o=s
        path=import_path(s,'mathMLTree')
        if path and ('mathMLTree[' in s or 'JS.invoke(mathMLTree' in s):
            used=[]
            for n in MT:
                if ('mathMLTree["%s"]'%n) in s:
                    if declares(s,n) and not re.search(r'import \{[^}]*\b'+n+r'\b',s): raise SystemExit('clash %s in %s'%(n,p))
                    c=s.count('mathMLTree["%s"]'%n); s=s.replace('mathMLTree["%s"]'%n,n); used.append(n); bump('mathMLTree.'+n,c)
            def rep_mi(m):
                n=m.group(1); used.append(n) if n not in used else None; bump('mathMLTree.invoke'); return 'JS.call(%s, undef()%s'%(n,m.group(2))
            s=re.sub(r'JS\.invoke\(mathMLTree, "([A-Za-z]+)"(\s*[,)])',rep_mi,s)
            if 'mathMLTree[' in s: raise SystemExit('other mathMLTree read in '+p)
            s=add_import(s,used,path)
        path=import_path(s,'buildCommon')
        if path and 'buildCommon' in s:
            used=set()
            orig=s
            def rep_inv(m):
                n=m.group(1)
                if declares(orig,n): bump('buildCommon.skip-localalias'); return m.group(0)
                used.add(n); bump('buildCommon.invoke'); return 'JS.call(%s, undef()%s'%(n, m.group(2))
            s=re.sub(r'JS\.invoke\(buildCommon, "([A-Za-z]+)"(\s*[,)])',rep_inv,s)
            def rep_rd(m):
                n=m.group(1)
                if declares(orig,n): bump('buildCommon.skip-localalias'); return m.group(0)
                used.add(n); bump('buildCommon.read'); return n
            s=re.sub(r'buildCommon\["([A-Za-z]+)"\]',rep_rd,s)
            for n in used:
                if declares(s,n): raise SystemExit('clash %s in %s'%(n,p))
            if re.search(r'\bbuildCommon\b(?!\s*\}| as|,)',s.split('\n',50)[-1]) and False: pass
            s=add_import(s,sorted(used),path); bc_names|=used
            if used and not re.search(r'import \{[^}]*\bundef\b',s): s=add_import(s,['undef'],path.replace('buildCommon.lil','host.lil')); bump('undef-import')
        if s!=o: wr(p,s)
    # export the used buildCommon functions
    m=re.search(r'export \{ __star, buildCommon as buildCommon \};',bs)
    bs=bs.replace(m.group(0),'export { __star, buildCommon as buildCommon, '+', '.join(sorted(bc_names))+' };'); wr(bp,bs)
    bump('buildCommon.exported',len(bc_names))
if 'C' in flags:
    sp=os.path.join(root,'Style.lil'); ss=rd(sp)
    ids={'D':0,'Dc':1,'T':2,'Tc':3,'S':4,'Sc':5,'SS':6,'SSc':7}
    for k,v in ids.items():
        if 'LIT' in flags:
            ss=ss.replace('JsValue %s = undef();\n'%k,'',1); ss=ss.replace('\n%s = %d;\n'%(k,v),'\n',1)
        else:
            ss=ss.replace('JsValue %s = undef();\n'%k,'int %s = %d;\n'%(k,v),1)
            ss=ss.replace('\n%s = %d;\n'%(k,v),'\n',1)
    if 'LIT' in flags:
        head,sep,tail=ss.partition('JsValue a1 = ')
        tail=re.sub(r'\b(SSc|SS|Sc|S|Tc|T|Dc|D)\b(?!")',lambda m:str(ids[m.group(1)]),tail)
        ss=head+sep+tail; bump('style.literal-ids')
    ix=(0,2,4,6) if 'LIT' in flags else ('D','T','S','SS')
    ss=ss.replace('Style = o8;\n','Style = o8;\nJsValue DISPLAY = styles[%s];\nJsValue TEXT = styles[%s];\nJsValue SCRIPT = styles[%s];\nJsValue SCRIPTSCRIPT = styles[%s];\n'%ix,1)
    ss=re.sub(r'export \{([^}]*)\};',lambda m:'export {'+m.group(1).rstrip()+', DISPLAY, TEXT, SCRIPT, SCRIPTSCRIPT };',ss,1)
    wr(sp,ss)
    for p in files:
        if p==sp: continue
        s=rd(p); o=s; path=import_path(s,'Style')
        if not path or 'Style[' not in s: continue
        used=[]
        for n in ['DISPLAY','TEXT','SCRIPT','SCRIPTSCRIPT']:
            c=s.count('Style["%s"]'%n)
            if c:
                if declares(s,n): raise SystemExit('clash %s in %s'%(n,p))
                s=s.replace('Style["%s"]'%n,n); used.append(n); bump('Style.'+n,c)
        if 'Style[' in s: raise SystemExit('other Style read in '+p)
        s=add_import(s,used,path)
        if s!=o: wr(p,s)
if 'D' in flags:
    for p in files:
        if p.endswith('host.lil'): continue
        s=rd(p); o=s
        decl_re=lambda x: re.compile(r'\b(?:JsValue|number|int|bool|string)\s+'+x+r'\b(\s*=\s*([^;]*))?')
        def ok(x,pos):
            ds=[m for m in decl_re(x).finditer(o) if m.start()<pos]
            if not ds: return False
            d=ds[-1]; init=(d.group(2) or '').strip()
            if not d.group(1): return False           # parameter
            if not (init.startswith('JS.array(') or init.startswith('newArray(') or init=='undef()'): return False
            nxt=[m.start() for m in decl_re(x).finditer(o) if m.start()>d.start()]
            end=nxt[0] if nxt else len(o)
            for m in re.finditer(r'(?<![\w."\[])'+x+r'\s*=(?!=)\s*([^;]*);',o[d.end():end]):
                if not re.match(r'(JS\.array|newArray)\(',m.group(1)): return False
            return True
        def rep(m):
            x=m.group(1)
            if ok(x,m.start()): bump('push.local'); return 'JS.invoke(%s, "push", '%x
            bump('push.skipped'); return m.group(0)
        s=re.sub(r'JS\.push\(([A-Za-z_][A-Za-z0-9_]*), ',rep,s)
        if s!=o: wr(p,s)
print(stats)
