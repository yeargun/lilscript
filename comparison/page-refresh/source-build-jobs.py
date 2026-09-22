#!/usr/bin/env python3
import json,pathlib,subprocess,sys,shutil
run=pathlib.Path(sys.argv[1]);old=pathlib.Path('/tmp/lilscript-page-refresh-20260910');xs=json.loads((run/'upstream-sources.json').read_text());jobs={}
compiler=subprocess.check_output(['git','rev-parse','HEAD'],cwd=run/'compiler',text=True).strip()
for x in xs:
 n=x['name'];u=pathlib.Path(x['sourcePath']);root=json.loads((u/'package.json').read_text());pkg=x['package'];pdir='.'
 for f in [u/'package.json',*u.glob('packages/*/package.json')]:
  if f.exists() and json.loads(f.read_text()).get('name')==pkg and (f.parent!=u or root.get('version')==x['version']):pdir=str(f.parent.relative_to(u))
 pmeta=json.loads((u/pdir/'package.json').read_text());manager=root.get('packageManager','npm').split('@')[0]
 install=['corepack',manager,'install','--frozen-lockfile','--ignore-scripts'] if manager=='pnpm' else ['corepack','yarn','install','--immutable','--mode=skip-builds'] if manager=='yarn' else ['npm','ci' if (u/'package-lock.json').exists() else 'install','--ignore-scripts','--no-audit','--no-fund']
 native=['npm','run','build'];clean=[];setup=[]
 entry=pmeta.get('module') or pmeta.get('exports') or 'index.js'
 def resolve_export(value):
  if isinstance(value,str):return value
  if isinstance(value,dict):
   for k in ['.','browser','import','production','default']:
    if k in value:
     result=resolve_export(value[k])
     if result:return result
  return None
 entry=str(pathlib.Path(pdir)/(resolve_export(entry) or 'index.js'))
 if manager=='pnpm':native=['corepack','pnpm','run','build']
 if manager=='yarn':native=['corepack','yarn','build']
 if n=='cnlil':native=['corepack','pnpm','--filter','cn','build'];clean=['packages/cn/dist']
 if n=='jquerylil':native=['npm','run','build-all-variants'];entry='dist/jquery.js';clean=['dist']
 if n=='markedlil':clean=['lib']
 if n=='mobxlil':native=['npm','run','build','--workspace','mobx','--','--environment','TARGET:publish'];entry='packages/mobx/dist/mobx.esm.js';clean=['packages/mobx/dist']
 if n=='solidlil':native=['corepack','pnpm','exec','turbo','run','build','--filter=solid-js...','--force'];entry='packages/solid/dist/solid.js';clean=['packages/solid/dist']
 if n=='motionlil':native=['corepack','yarn','turbo','run','build','--filter=motion...','--force'];entry='packages/motion-dom/dist/es/index.mjs';clean=['packages/motion/dist','packages/framer-motion/dist','packages/motion-dom/dist','packages/motion-utils/dist']
 if n=='posthoglil':install+=['--filter','posthog-js...'];native=['corepack','pnpm','exec','turbo','run','build','--filter=posthog-js...','--force'];entry='packages/browser/lib/src/posthog.js';clean=['packages/browser/lib','packages/browser/dist'];
 if n=='zodlil':native=['corepack','pnpm','--filter','zod','build'];entry='packages/zod/v4/index.js'
 if n=='monacolil':setup=[['npm','ci','--prefix','monaco-lsp-client','--ignore-scripts','--no-audit','--no-fund'],['npm','run','build','--prefix','monaco-lsp-client'],['npm','run','postinstall']];native=['npm','run','build-monaco-editor'];entry='out/monaco-editor/esm/vs/index.js';clean=['out/monaco-editor']
 if n=='vuelil':native=['corepack','pnpm','build','--formats','esm-bundler'];entry='packages/vue/dist/vue.runtime.esm-bundler.js'
 if n=='playcanvaslil':native=['npm','run','build:rel:esm'];entry='build/playcanvas/src/index.js';clean=['build/playcanvas']
 config=json.loads((old/'current'/n/'page-audit-config.json').read_text());snapshot=old/'current'/n
 public=json.loads((old/'publications.json').read_text())[n]
 jobs[n]={'source':{'repository':x['sourceUrl'],'commit':x['sourceCommit'],'package':pkg,'version':x['version'],'directory':pdir},'portSource':{'commit':config['baseCommit'],'snapshot':str(snapshot)},'compilerCommit':compiler,'install':install,'setup':setup,'build':native,'clean':clean,'entry':entry,'defaultExport':config.get('upstreamEntry','').find('default')>=0,'external':['react','react/jsx-runtime','react-dom'] if n=='react-markdownlil' else ['katex'] if n=='rehype-katexlil' else [],'esmScope':config.get('upstreamScope'),'lilscriptBuild':config['buildCommand'],'test':config.get('testCommand'),'publication':public}
(run/'jobs.json').write_text(json.dumps(jobs,indent=2)+'\n')
print(len(jobs),'paired source-build jobs prepared')
