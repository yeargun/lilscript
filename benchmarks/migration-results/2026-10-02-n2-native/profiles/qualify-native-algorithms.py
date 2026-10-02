#!/usr/bin/env python3
"""Unchanged algorithm sources, runtime vector providers, serial native/JS profiles."""
from pathlib import Path
import subprocess, json, os, sys, time, resource, statistics, hashlib, re

root = Path(__file__).resolve().parents[4]
os.chdir(root)
compiler = str(Path(sys.argv[1]).resolve())
work = Path(sys.argv[2]).resolve(); work.mkdir(parents=True, exist_ok=True)
node = os.environ.get('N2_NODE', '/home/azureuser/.npm/_npx/387698761821791d/node_modules/node/bin/node')
clang = os.environ.get('N2_CLANG', '/home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18')
esbuild = str(root / 'benchmarks/popular/node_modules/.bin/esbuild')
rows, measurements, sources = [], [], []

def run(name, args, expected=None, trace=None, env=None):
    before = resource.getrusage(resource.RUSAGE_CHILDREN); start = time.monotonic()
    p = subprocess.run(args, capture_output=True, timeout=120, env={**os.environ, **(env or {})})
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    sample = dict(wall_ms=(time.monotonic()-start)*1000, cpu_ms=(after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime)*1000)
    ok = p.returncode == 0 and (expected is None or p.stdout == expected) and (trace is None or p.stderr == trace)
    rows.append(dict(name=name, args=args, env=env or {}, exit=p.returncode, passed=ok, **sample))
    (work / (name+'.stdout')).write_bytes(p.stdout); (work / (name+'.log')).write_bytes(p.stderr)
    if not ok: print(name, p.returncode, p.stdout.decode(errors='replace')[-500:], p.stderr.decode(errors='replace')[-2000:], flush=True)
    assert ok, name
    return p, sample

def c_provider(vectors, names):
    text = '#include "program.h"\n#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n'
    for v, vector in enumerate(vectors):
        ints = ','.join(str(i) for i in vector['ints']) or '0'
        text += f'static const int32_t ints{v}[] = {{{ints}}};\n'
        for s, value in enumerate(vector['strings']):
            b = value.encode('utf-16le', errors='surrogatepass')
            units = ','.join(str(int.from_bytes(b[i:i+2],'little')) for i in range(0,len(b),2)) or '0'
            text += f'static const uint16_t text{v}_{s}[] = {{{units}}};\n'
        strings = ','.join(f'{{text{v}_{s},{len(value.encode("utf-16le",errors="surrogatepass"))//2},NULL}}' for s,value in enumerate(vector['strings'])) or '{NULL,0,NULL}'
        text += f'static const ls_string strings{v}[] = {{{strings}}};\n'
    text += 'typedef struct {const int32_t *ints;size_t ni;const ls_string *strings;size_t ns;} vector;\nstatic const vector vectors[]={\n'
    text += ',\n'.join(f'{{ints{v},{len(x["ints"])},strings{v},{len(x["strings"])}}}' for v,x in enumerate(vectors)) + '};\n'
    text += r'''
static const vector *input;
static int repeat, trace, calls;
static uint16_t **expanded;
static void finish(void) {if(trace) fputs("]\n",stderr);if(expanded){for(size_t i=0;i<input->ns;i++)free(expanded[i]);free(expanded);}}
static void boot(void) {
    if(input)return;
    const char *at=getenv("N2_VECTOR"), *times=getenv("N2_REPEAT");
    int index=at?atoi(at):0;repeat=times?atoi(times):1;
    if(index<0 || (size_t)index>=sizeof vectors/sizeof *vectors || repeat<1)abort();
    input=&vectors[index];trace=getenv("N2_TRACE")!=NULL;
    if(trace)fputs("LILSCRIPT_ALGORITHM_TRACE=[",stderr);
    if(atexit(finish))abort();
}
static void observe(const char *name,int index) {
    boot();if(!trace)return;if(calls++)fputc(',',stderr);
    if(index<0)fprintf(stderr,"[\"%s\"]",name);else fprintf(stderr,"[\"%s\",%d]",name,index);
}
'''
    if 'algorithmCount' in names:
        text += 'int32_t host_algorithmCount(void){boot();observe("count",-1);return (int32_t)((input->ni>input->ns?input->ni:input->ns)*(size_t)repeat);}\n'
    if 'algorithmInt' in names:
        text += 'int32_t host_algorithmInt(int32_t index){boot();if(index<0 || (size_t)index>=input->ni*(size_t)repeat)abort();observe("int",index);return input->ints[(size_t)index%input->ni];}\n'
    if 'algorithmString' in names:
        if 'algorithmCount' in names:
            text += 'ls_string host_algorithmString(int32_t index){boot();if(index<0 || (size_t)index>=input->ns*(size_t)repeat)abort();observe("string",index);return input->strings[(size_t)index%input->ns];}\n'
        else:
            text += r'''
ls_string host_algorithmString(int32_t index){
    boot();if(index<0 || (size_t)index>=input->ns)abort();observe("string",index);
    ls_string value=input->strings[index];if(repeat==1)return value;
    if(!expanded){expanded=calloc(input->ns,sizeof *expanded);if(!expanded)abort();}
    if(!expanded[index]){size_t bytes=value.length*sizeof(uint16_t);expanded[index]=malloc(bytes*(size_t)repeat+1);if(!expanded[index])abort();for(int n=0;n<repeat;n++)memcpy(expanded[index]+(size_t)n*value.length,value.data,bytes);}
    return (ls_string){expanded[index],value.length*(size_t)repeat,NULL};
}
'''
    return text

try:
    for manifest in sorted((root/'comparison/algorithms/cases').glob('*/case.json')):
        case = json.loads(manifest.read_text()); name=case['id']; directory=manifest.parent
        d=work/name;d.mkdir(exist_ok=True); vectors=case['vectors']
        names=sorted(set(re.findall(r'extern\s+(?:int|string)\s+(algorithm\w+)\s*\(', '\n'.join(p.read_text() for p in directory.glob('*.lil')))))
        assert set(names)<= {'algorithmCount','algorithmString','algorithmInt'}
        sources.append(dict(case=name, files=[dict(path=str(p.relative_to(root)), sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in sorted(directory.iterdir()) if p.suffix in ['.lil','.js','.json']]))
        cfg=d/'config.toml';cfg.write_text('effort.level=13\nobjective.codecs="raw"\n[host.native]\n'+''.join(f'{n}="host_{n}"\n' for n in names))
        c=d/'program.c';js=d/'program.js';original=d/'original.js'
        run(name+'-source-native',[compiler,str(directory/'main.lil'),'--config',str(cfg),'--target','c','-o',str(c)])
        run(name+'-source-js',[compiler,str(directory/'main.lil'),'--config',str(cfg),'--target','js','--format','bare','-o',str(js)])
        run(name+'-bundle-original',[esbuild,str(directory/'main.js'),'--bundle','--format=iife','--platform=node','--log-level=error','--outfile='+str(original)])
        host=d/'host.c';host.write_text(c_provider(vectors,names))
        launcher=d/'run.cjs';launcher.write_text('''const vectors='''+json.dumps(vectors)+''';
const source=vectors[Number(process.env.N2_VECTOR||0)],repeat=Number(process.env.N2_REPEAT||1),counted='''+json.dumps('algorithmCount' in names)+''';
const input={ints:Array.from({length:source.ints.length*repeat},(_,i)=>source.ints[i%source.ints.length]),strings:counted?Array.from({length:source.strings.length*repeat},(_,i)=>source.strings[i%source.strings.length]):source.strings.map(x=>x.repeat(repeat))};
const trace=process.env.N2_TRACE!==undefined,events=[];
if(trace)process.once('exit',()=>process.stderr.write('LILSCRIPT_ALGORITHM_TRACE='+JSON.stringify(events)+'\\n'));
globalThis.algorithmCount=()=>{if(trace)events.push(['count']);return Math.max(input.ints.length,input.strings.length)};
globalThis.algorithmInt=index=>{if(!Number.isInteger(index)||index<0||index>=input.ints.length)throw Error('int bounds');if(trace)events.push(['int',index]);return input.ints[index]};
globalThis.algorithmString=index=>{if(!Number.isInteger(index)||index<0||index>=input.strings.length)throw Error('string bounds');if(trace)events.push(['string',index]);return input.strings[index]};
require(process.argv[2]);
''')
        lanes=[]
        for lane,cc,flags in [('speed','cc',['-O3']),('balanced','cc',['-O2']),('size','cc',['-Os']),('sanitize',clang,['-O2','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            exe=d/lane
            run(name+'-'+lane+'-build',[cc,'-std=c11','-fno-fast-math','-ffp-contract=off','-Wall','-Wextra','-Werror',*flags,str(c),str(host),'-lm','-o',str(exe)])
            lanes.append((lane,[str(exe)],exe))
        lanes.append(('javascript',[node,str(launcher),str(js)],js))
        for index, vector in enumerate(vectors):
            env={'N2_VECTOR':str(index),'N2_TRACE':'1'}
            oracle,_=run(f'{name}-oracle-{index}',[node,str(launcher),str(original)],vector['expected'].encode(),env=env)
            for lane,args,_ in lanes:run(f'{name}-{lane}-vector-{index}',args,oracle.stdout,oracle.stderr,env)
        env={'N2_VECTOR':'0','N2_REPEAT':'2048'}
        oracle,_=run(name+'-expanded-oracle',[node,str(launcher),str(original)],env=env)
        # Interleave the profile samples. All include startup, host access,
        # allocation and output. No claim about isolated steady-state kernels.
        samples={lane:[] for lane,_,_ in lanes}
        for iteration in range(5):
            for lane,args,_ in (lanes if iteration%2==0 else list(reversed(lanes))):
                if lane=='sanitize' and iteration:continue
                _,sample=run(f'{name}-{lane}-sample-{iteration}',args,oracle.stdout,env=env);samples[lane].append(sample)
        for lane,_,artifact in lanes:
            measurements.append(dict(case=name,lane=lane,repeat=2048,samples=samples[lane],median_cpu_ms=statistics.median(s['cpu_ms'] for s in samples[lane]),median_wall_ms=statistics.median(s['wall_ms'] for s in samples[lane]),artifact_bytes=artifact.stat().st_size))
        print(name,'passed',flush=True)
finally:
    (work/'qualification.json').write_text(json.dumps(dict(schema=1,scope='Eleven unchanged portable algorithm graphs; exact vector stdout and host-call order; expanded first vector repeated 2048 times for five alternating end-to-end process samples, including Node startup. Sanitizer samples are correctness only.',compiler=dict(path=compiler,sha256=hashlib.sha256(Path(compiler).read_bytes()).hexdigest()),sources=sources,rows=rows,measurements=measurements),indent=2)+'\n')
