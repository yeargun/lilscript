#!/usr/bin/env python3
"""Verify exact-commit Pages workflows and live receipts, preserving concurrent updates."""
import concurrent.futures
import datetime
import fcntl
import hashlib
import json
import pathlib
import subprocess
import sys
import urllib.request

run=pathlib.Path(sys.argv[1]).resolve()
path=run/'publications.json'
publications=json.loads(path.read_text())
if '--pending' in sys.argv:
    publications={name:value for name,value in publications.items() if value.get('deployment')!='success'}


def fetch(url):
    with urllib.request.urlopen(urllib.request.Request(url,headers={'Cache-Control':'no-cache'}),timeout=40) as response:
        return response.read()


def check(item):
    name,record=item
    if record.get('status')!='pushed':return
    workflow=json.loads(subprocess.check_output(['gh','run','list','--repo','yeargun/'+name,'--limit','10','--json','databaseId,name,status,conclusion,headSha,url'],text=True))
    current=next((entry for entry in workflow if entry['headSha']==record['commit'] and 'pages' in entry['name'].lower()),{})
    result={'checkedAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),'workflow':current,'deployment':current.get('conclusion') or current.get('status','pending')}
    if current.get('conclusion')=='success':
        root=pathlib.Path(record['path']);site=root/('web' if name=='vuelil' else 'site')
        informational=record.get('presentation')=='informational'
        filename='comparison.json' if informational else 'build-audit.json'
        expected=(site/filename).read_bytes()
        base='https://yeargun.github.io/'+name+'/'
        query='?audit='+record['commit']
        try:
            actual=fetch(base+filename+query)
            if json.loads(actual)!=json.loads(expected):raise RuntimeError('Live receipt differs from the published commit')
            html=fetch(base+query).decode()
            marker='id="build-comparison"' if informational else 'id="build-audit"'
            if marker not in html or 'Standard_D16als_v7' not in html:raise RuntimeError('Live page is missing its build-machine facts')
            if informational and any(value in html for value in ['id="build-audit"','class="compiler-progress','data-audit-history']):raise RuntimeError('Live page still contains audit history')
            result.update(deployment='success',liveReceiptSha256=hashlib.sha256(actual).hexdigest(),liveUrl=base)
        except Exception as error:result.update(deployment='live verification pending',error=str(error))
    with (run/'publications.lock').open('w') as handle:
        fcntl.flock(handle,fcntl.LOCK_EX)
        latest=json.loads(path.read_text())
        if latest.get(name,{}).get('commit')==record['commit']:
            latest[name].update(result)
            path.write_text(json.dumps(latest,indent=2)+'\n')
    print(name,result['deployment'],result.get('error',''),flush=True)


with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
    for value in pool.map(check,publications.items()):pass
