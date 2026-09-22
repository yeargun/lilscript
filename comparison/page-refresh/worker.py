#!/usr/bin/env python3
"""Run one isolated port on a worker, recording failures as evidence."""
import glob
import hashlib
import json
import os
import pathlib
import platform
import re
import shutil
import signal
import subprocess
import sys
import time
import threading

ROOT = pathlib.Path(sys.argv[1]).resolve()
TOOLS = pathlib.Path(__file__).resolve().parent
CONFIG = json.loads((ROOT / "page-audit-config.json").read_text())
OUT = ROOT / ".page-audit"
OUT.mkdir(exist_ok=True)
heartbeat = pathlib.Path.home()/'lil/.heartbeat'
def keep_alive():
    while True:
        heartbeat.parent.mkdir(exist_ok=True)
        heartbeat.touch()
        time.sleep(30)
threading.Thread(target=keep_alive, daemon=True).start()
env = os.environ.copy()
compiler = str(TOOLS / "lilscript")
wrapper = str(TOOLS / "compiler-wrapper.py")
env.update({"LILSCRIPT_COMPILER": wrapper, "MOTIONLIL_LILSCRIPT_BIN": wrapper,
            "SOLIDLIL_LILSCRIPT_BIN": wrapper, "LILSCRIPT_CODEC": str(TOOLS / "lilscript-codec"),
            "LILSCRIPT_ROOT": str(ROOT.parent / "lilscript"), "RAYON_NUM_THREADS": "16",
            "LILSCRIPT_TIMING": "1", "PAGE_AUDIT_REAL_COMPILER": compiler,
            "PAGE_AUDIT_INVOCATIONS": str(OUT / "invocations.jsonl")})
sha = lambda path: hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()
machine = {"cpu": next((line.split(":", 1)[1].strip() for line in pathlib.Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')), platform.processor()),
           "logicalCpus": os.cpu_count(), "affinityCpus": len(os.sched_getaffinity(0)),
           "memoryBytes": os.sysconf('SC_PAGE_SIZE') * os.sysconf('SC_PHYS_PAGES'),
           "os": platform.freedesktop_os_release().get('PRETTY_NAME'), "kernel": platform.release(),
           "architecture": platform.machine(), "node": subprocess.check_output(['node','--version'], text=True).strip(),
           "rayonThreads": 16, "provider": "Azure", "instanceClass": "Standard_D16als_v7",
           "parallelPortBuildsOnMachine": 1, "loadAtStart": os.getloadavg()}
result = {"schemaVersion": 1, "name": CONFIG['name'], "sourceCommit": CONFIG['baseCommit'],
          "startedAt": time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()), "machine": machine,
          "compiler": {"commit": CONFIG['compilerCommit'], "sha256": sha(compiler), "profile": "release", "dirty": False},
          "codecSha256": sha(TOOLS / 'lilscript-codec')}


def save():
    (OUT / 'result.json').write_text(json.dumps(result, indent=2) + '\n')


def run(label, args, cwd=ROOT, timeout=3600):
    print(CONFIG['name'], label, flush=True)
    started = time.monotonic()
    logfile = OUT / (label + '.log')
    with logfile.open('w') as log:
        child = subprocess.Popen(args, cwd=cwd, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            code = child.wait(timeout=timeout)
            timed_out = False
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            code = child.wait()
            timed_out = True
    text = logfile.read_text(errors='replace')
    failures = sorted(set(re.findall(r'^\s*not ok \d+\s*-?\s*(.+)$', text, re.M)))
    record = {"command": args, "exitCode": code, "wallSeconds": time.monotonic()-started,
              "timedOut": timed_out, "failures": failures, "tail": text[-5000:], "logSha256": sha(logfile)}
    result[label] = record
    save()
    return code == 0


save()
if not run('install', ['npm', 'ci', '--no-audit', '--no-fund'], timeout=600):
    if CONFIG['name'] == 'mobxlil' and 'Missing:' in result['install']['tail']:
        if not run('repair-lock', ['npm','install','--package-lock-only','--ignore-scripts','--no-audit','--no-fund'], timeout=600): sys.exit(1)
        if not run('install-retry', ['npm','ci','--no-audit','--no-fund'], timeout=600): sys.exit(1)
        result['lockfileRepair'] = 'npm ci rejected missing optional dependency entries; npm install --package-lock-only repaired the lock, followed by successful npm ci.'
    else:
        sys.exit(1)
if (ROOT/'node_modules/playwright/package.json').exists():
    run('browser-setup', ['npx','playwright','install','--with-deps','chromium'], timeout=600)
if (ROOT / '.gitmodules').exists():
    # Submodule trees were exported at their gitlink revisions by the controller.
    result['submodules'] = CONFIG.get('submodules', [])

if CONFIG.get('upstreamPackage'):
    baseline = ROOT / 'benchmarks/page-audit'
    baseline.mkdir(parents=True, exist_ok=True)
    (baseline / 'package.json').write_text(json.dumps({"private": True, "type": "module", "dependencies": {
        CONFIG['upstreamPackage']: CONFIG['upstreamVersion'], "esbuild": "0.28.1", "terser": "5.51.2"}}, indent=2) + '\n')
    if run('upstream-install', ['npm', 'install', '--no-audit', '--no-fund'], cwd=baseline, timeout=600):
        result['upstreamLockSha256'] = sha(baseline/'package-lock.json')
        if run('upstream-build', ['node', str(TOOLS/'upstream.mjs'), str(ROOT)], timeout=600):
            result['upstreamTiming'] = json.loads((OUT/'upstream/timing.json').read_text())
            result['upstreamArtifactSha256'] = sha(OUT/'upstream/official.esm.js')

if CONFIG.get('setup'):
    run('setup', ['bash','-c',CONFIG['setup']], timeout=1200)

test = CONFIG.get('testCommand')
if test:
    run('before-tests', ['bash','-c',test], timeout=900)

# Preserve the baseline, then delete every JS distribution artifact before compilation.
# This cannot silently measure an old dist after a failed/skipped build.
if (ROOT/'dist').exists():
    shutil.copytree(ROOT/'dist', OUT/'before-dist', dirs_exist_ok=True)
    for path in (ROOT/'dist').rglob('*'):
        if path.is_file() and path.suffix in ['.js','.mjs','.cjs','.map']:
            path.unlink()
invocations = OUT/'invocations.jsonl'
if invocations.exists(): invocations.unlink()
ok = run('build', ['bash','-c',CONFIG['buildCommand']], timeout=5400)
result['invocations'] = [json.loads(line) for line in invocations.read_text().splitlines()] if invocations.exists() else []
result['build']['compilerWasInvoked'] = bool(result['invocations'])
ok = ok and bool(result['invocations'])
result['build']['fresh'] = ok
if (ROOT/'reports/build-timings.json').exists():
    result['bundleTimings'] = json.loads((ROOT/'reports/build-timings.json').read_text())
save()
if ok:
    artifacts = [str(path) for path in (ROOT/'dist').rglob('*') if path.is_file() and path.suffix in ['.js','.mjs','.cjs']]
    if CONFIG['name'] == 'vuelil': artifacts += [str(path) for path in (ROOT/'artifacts').glob('*.generated.js')]
    artifacts += [str(path) for path in (OUT/'upstream').glob('*.js')]
    if artifacts:
        proc = subprocess.run([str(TOOLS/'lilscript-codec'),'--json',*artifacts], capture_output=True, text=True)
        if proc.returncode == 0:
            measured = json.loads(proc.stdout)
            result['codecs'] = measured['codecs']
            result['artifacts'] = [{**row, 'path': str(pathlib.Path(row['path']).relative_to(ROOT)), 'sha256': sha(row['path'])} for row in measured['artifacts']]
    save()
    # Update raw/gzip/Brotli receipts consumed by existing site tests; runtime rows remain historical.
    existing = ROOT/'site/results.json'
    if existing.exists() and CONFIG.get('file'):
        data = json.loads(existing.read_text())
        by_path = {row['path']: row for row in result.get('artifacts',[])}
        for lane in data.get('size',[]):
            identifier = lane.get('id','')
            mapping = {'itslil': f"dist/{CONFIG['file']}.esm.js", 'itslil-closed': f"dist/{CONFIG['file']}.closed.js"}
            if identifier in mapping and mapping[identifier] in by_path:
                lane.update({key:by_path[mapping[identifier]][key] for key in ['raw','gzip9','brotli11']})
        existing.write_text(json.dumps(data, indent=2)+'\n')
    if test:
        shutil.rmtree(ROOT/'_site', ignore_errors=True)
        run('after-tests', ['bash','-c',test], timeout=1200)
    result['machine']['loadAtEnd'] = os.getloadavg()
    save()
print(CONFIG['name'], 'finished', flush=True)
