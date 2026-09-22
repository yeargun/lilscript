#!/usr/bin/env python3
"""Dispatch isolated snapshots, one port per Azure worker, and collect receipts."""
import concurrent.futures
import hashlib
import json
import pathlib
import queue
import re
import shlex
import subprocess
import sys
import tarfile
import time
import os

RUN = pathlib.Path(sys.argv[1]).resolve()
TOOLS = pathlib.Path(__file__).resolve().parent
CURRENT = '--current' in sys.argv
REMOTE = 'page-refresh-current-20260910' if CURRENT else 'page-refresh-20260910'
SSH = ['ssh', '-o', 'BatchMode=yes', '-o', 'StrictHostKeyChecking=accept-new', '-o', 'UserKnownHostsFile='+str(RUN/'known_hosts'), '-o', 'ConnectTimeout=15', '-o', 'ServerAliveInterval=30']
rows = json.loads((RUN/'prepared-current.json' if CURRENT else RUN/'prepared.json').read_text())
compiler_root = RUN/('current' if CURRENT else 'repos')/'lilscript'
compiler_commit = subprocess.check_output(['git','-C',str(compiler_root),'rev-parse','HEAD'], text=True).strip()
os.environ['RSYNC_RSH'] = shlex.join(SSH)
claims = RUN/('current-claims' if CURRENT else 'claims')
claims.mkdir(exist_ok=True)
fallback = {'cnlil':('cn','0.2.4'), 'motionlil':('motion','13.1.0'), 'solidlil':('solid-js','2.0.0-rc.0'), 'vuelil':('vue','3.5.42')}


def run(args, **kw):
    subprocess.run(args, check=True, **kw)


def prepare(row):
    root = pathlib.Path(row['snapshot'])
    package = row['package']
    data = {}
    for path in [root/'site/results.json', RUN/'before'/row['name']/'results.json']:
        try: data = json.loads(path.read_text()); break
        except (FileNotFoundError,json.JSONDecodeError): pass
    pin = data.get('pin')
    upstream = tuple(pin.rsplit('@',1)) if pin and '@' in pin else fallback.get(row['name'])
    test = package.get('scripts',{}).get('test')
    if test:
        test = re.sub(r'^(?:npm run build|node scripts/build\.mjs[^&]*)\s*&&\s*', '', test)
    if row['name']=='mobxlil': test = 'node --experimental-vm-modules node_modules/jest/bin/jest.js --config jest.config.cjs --runInBand'
    if row['name']=='vuelil':
        components = ['shared','reactivity','runtime-core','runtime-dom','runtime-test','compiler-core','compiler-dom','vue-compat','compiler-ssr','server-renderer','vue','compiler-sfc','compatibility']
        build = ' && '.join('npm run build:'+name for name in components)
    else:
        build = 'node scripts/build.mjs --compile --force' + (' --prod' if row['name']=='zodlil' else '')
    config = {'name':row['name'],'baseCommit':row['baseCommit'],'compilerCommit':compiler_commit,
              'testCommand':test,'buildCommand':build,'file':data.get('file'), 'submodules':[],
              'primaryArtifact':package.get('module')}
    if row['name']=='vuelil':
        config['setup']='npm run setup:upstream && corepack pnpm --dir upstream/vue build shared reactivity --formats esm-browser'
    if upstream:
        pkg, version = upstream
        default = data.get('officialExport') == 'default' or pkg in ['jquery','katex']
        config.update({'upstreamPackage':pkg,'upstreamVersion':version,
                       'upstreamEntry':f'export * from "{pkg}";'+(f' export {{ default }} from "{pkg}";' if default else ''),
                       'upstreamScope': f'{pkg}@{version} public ESM runtime graph; dependency installation excluded; this is a comparison bundle build, not the upstream vendor release pipeline'})
        if row['name']=='markedlil': config['upstreamScope']='Pinned Marked parse-only source graph via scripts/official-parse-bundle.mjs; excludes extension API'
        if row['name']=='posthoglil': config['upstreamScope']='Pinned PostHog selected kernel source modules via scripts/official-bundle.mjs; not the complete SDK'
        if row['name']=='react-markdownlil': config['external']=['react','react/jsx-runtime','react-dom']
    if (root/'.gitmodules').exists():
        listing=subprocess.check_output(['git','-C',str(root),'ls-files','--stage'], text=True)
        for line in listing.splitlines():
            if not line.startswith('160000 '): continue
            before,path=line.split('\t',1); commit=before.split()[1]
            source=pathlib.Path(row['path'])/path
            dest=root/path
            if source.exists():
                content=subprocess.check_output(['git','-C',str(source),'archive',commit])
                archive=RUN/(row['name']+'-submodule.tar');archive.write_bytes(content)
                with tarfile.open(archive) as tf: tf.extractall(dest, filter='data')
                archive.unlink()
                config['submodules'].append({'path':path,'commit':commit})
    (root/'page-audit-config.json').write_text(json.dumps(config,indent=2)+'\n')
    return row


for row in rows:
    if not (pathlib.Path(row['snapshot'])/'page-audit-config.json').exists(): prepare(row)
pending=queue.Queue()
# Start the largest source graphs first to keep the tail short.
priority=['vuelil','katexlil','rehype-katexlil','monacolil','rehypelil','remarklil','motionlil','micromarklil','mdast-util-from-markdownlil','posthoglil']
for row in sorted(rows,key=lambda r: priority.index(r['name']) if r['name'] in priority else len(priority)):
    pending.put(row)


def worker(ip):
    host='lilfarm@'+ip
    remote_tools=REMOTE+'/tools'
    run([*SSH,host,'mkdir -p '+remote_tools+' '+REMOTE+'/repos/lilscript/target/release'])
    run(['rsync','-a',str(TOOLS)+'/',host+':'+remote_tools+'/'])
    binaries=compiler_root/'target/release'
    run(['rsync','-a',str(binaries/'lilscript'),str(binaries/'lilscript-codec'),host+':'+remote_tools+'/'])
    run(['rsync','-a','--exclude=.git','--exclude=node_modules','--exclude=target',str(compiler_root)+'/',host+':'+REMOTE+'/repos/lilscript/'])
    run([*SSH,host,'chmod +x '+remote_tools+'/compiler-wrapper.py; cp '+remote_tools+'/lilscript* '+REMOTE+'/repos/lilscript/target/release/'])
    while True:
        try: row=pending.get_nowait()
        except queue.Empty: return
        try: (claims/row['name']).mkdir()
        except FileExistsError: continue
        root=pathlib.Path(row['snapshot']);name=row['name'];dest=REMOTE+'/repos/'+name
        print('START',name,ip,flush=True)
        try:
            run(['rsync','-a','--exclude=.git','--exclude=node_modules','--exclude=_site','--exclude=.page-audit','--exclude=target',str(root)+'/',host+':'+dest+'/'])
            # Vue requires Node 24; use the pinned npm-distributed binary in its own tool directory.
            prefix=''
            if name=='vuelil':
                run([*SSH,host,'npm install --prefix '+REMOTE+'/node24 node@24.11.1 --no-audit --no-fund >/dev/null 2>&1'])
                prefix='export PATH="$HOME/'+REMOTE+'/node24/node_modules/.bin:$PATH"; '
            with (RUN/(('current-' if CURRENT else '')+name+'-worker.log')).open('w') as log:
                process=subprocess.run([*SSH,host,prefix+'python3 '+remote_tools+'/worker.py '+dest], stdout=log, stderr=subprocess.STDOUT)
            # Collect only this isolated checkout, never the original sibling worktree.
            run(['rsync','-a','--exclude=node_modules','--exclude=.git',host+':'+dest+'/',str(root)+'/'])
            print('DONE',name,'exit='+str(process.returncode),flush=True)
        except Exception as error:
            print('ERROR',name,str(error),flush=True)
            (RUN/(name+'-dispatch-error.txt')).write_text(str(error))
        finally: pending.task_done()


hosts = sys.argv[sys.argv.index('--hosts')+1].split(',') if '--hosts' in sys.argv else ['10.1.0.20','10.1.0.21','10.1.0.22','10.1.0.17','10.1.0.18']
with concurrent.futures.ThreadPoolExecutor(max_workers=len(hosts)) as executor:
    futures=[executor.submit(worker,ip) for ip in hosts]
    for future in concurrent.futures.as_completed(futures): future.result()
