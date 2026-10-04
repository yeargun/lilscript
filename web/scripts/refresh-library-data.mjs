import {writeFileSync,mkdirSync,readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {dirname,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const definitions=[
 ['katexlil','KaTeX','Main ESM; fonts and CSS excluded.'],
 ['react-markdownlil','react-markdown','Markdown-to-React pipeline; React external.'],
 ['markedlil','Marked','Parse API; extension-only exports excluded.'],
 ['zodlil','Zod','Node package root; locale provider external.'],
 ['motionlil','Motion','Full Motion DOM entry; no React integration.'],
 ['jquerylil','jQuery','Browser singleton and jQuery API.'],
 ['posthoglil','PostHog','Seven submodules, each compared with PostHog’s own build of the same code; totals of the seven.'],
 ['monacolil','Monaco','Partial editor namespace; CSS and workers excluded.'],
 ['mobxlil','MobX','Production MobX 7 API.'],
 ['cnlil','cn','Main class-merging API, including its tables.'],
 ['playcanvaslil','PlayCanvas','Shader-processing subsystem; not the complete engine.'],
 ['solidlil','Solid compatibility runtime','Experimental Solid 2 compatibility, including JavaScript providers.'],
 ...['rehypelil','remarklil','micromarklil','mdast-util-from-markdownlil','remark-parselil','unifiedlil','mdast-util-to-hastlil','remark-rehypelil','rehype-katexlil','hast-util-to-htmllil','rehype-stringifylil','remark-gfmlil','remark-breakslil','remark-mathlil'].map(name=>[name,name.slice(0,-3),'Portable main-entry API; see the linked dependency boundary.']),
];
const hash=data=>createHash('sha256').update(data).digest('hex');
const downloads=new Map();
async function bytes(url){if(downloads.has(url))return downloads.get(url);const r=await fetch(url,{cache:'no-store',signal:AbortSignal.timeout(60000)});if(!r.ok)throw Error(`${r.status} ${url}`);const data=Buffer.from(await r.arrayBuffer());downloads.set(url,data);return data;}
async function json(url){const raw=await bytes(url);return {data:JSON.parse(raw),sha256:hash(raw)};}
const libraries=[];
for(const [name,title,scope] of definitions){
 const homepage=`https://yeargun.github.io/${name}/`;
 const special=name==='posthoglil';
 const evidence=new URL(special?'evidence.json':'comparison.json',homepage).href;
 const {data,sha256:evidenceSha256}=await json(evidence);
 let objectives,upstream,measuredAt,compilerSha256,timingScope;
 if(special){
  // posthoglil compares seven submodules, each on its own; the row is their totals. The original side
  // of each module is its smallest upstream build for that codec (PostHog's build, esbuild or Oxc).
  const rows=data.results.results, sum=f=>rows.reduce((a,r)=>a+f(r),0);
  for(const [href,sha256] of Object.entries(data.files))if(hash(await bytes(new URL(href,homepage).href))!==sha256)throw Error(`${name}: ${href} public bytes differ`);
  objectives=['raw','gzip','brotli'].map(objective=>({objective,
   originalBytes:sum(r=>r.best[objective].bytes),lilscriptBytes:sum(r=>r.lilscript[objective][objective]),
   originalTool:'smallest upstream per module',
   originalBuildSeconds:+sum(r=>r.upstream[r.best[objective].lane].seconds).toFixed(3),lilscriptBuildSeconds:+sum(r=>r.lilscript[objective].seconds).toFixed(2),
   originalArtifact:evidence,lilscriptArtifact:evidence,originalSha256:evidenceSha256,lilscriptSha256:evidenceSha256}));
  upstream=`posthog-js@${data.meta.upstreamVersion}`;measuredAt=data.results.date;compilerSha256=data.meta.compilerSha256;
  timingScope='Per submodule, summed: upstream bundling and minifier / LilScript compilation. Installation, tests and final compression excluded.';
 }else{
  if(data.schemaVersion!==4)throw Error(`${name}: current objective comparison missing`);
  objectives=data.objectives.map(row=>({objective:row.objective,originalBytes:row.original.sizes[row.metric],lilscriptBytes:row.lilscript.sizes[row.metric],originalTool:row.original.tool,originalBuildSeconds:row.original.buildSeconds,lilscriptBuildSeconds:row.lilscript.buildSeconds,originalArtifact:new URL(row.original.artifact,homepage).href,lilscriptArtifact:new URL(row.lilscript.artifact,homepage).href,originalSha256:row.original.sha256,lilscriptSha256:row.lilscript.sha256}));
  upstream=`${data.upstream.package}@${data.upstream.version}`;measuredAt=data.measuredAt;compilerSha256=data.compiler.binarySha256;
  timingScope='Original package bundling + minification / LilScript compilation. Installation, tests and final compression excluded.';
 }
 if(JSON.stringify(objectives.map(x=>x.objective))!==JSON.stringify(['raw','gzip','brotli']))throw Error(`${name}: objective inventory mismatch`);
 for(const row of objectives){
  for(const side of ['original','lilscript'])if(hash(await bytes(row[side+'Artifact']))!==row[side+'Sha256'])throw Error(`${name}/${row.objective}: ${side} public bytes differ`);
  if(!(row.originalBytes>0&&row.lilscriptBytes>0&&row.lilscriptBuildSeconds>0))throw Error(`${name}: incomplete measurement`);
 }
 const packageEvidence=new URL(special?'evidence.json':'package-build.json',homepage).href;
 const {data:packageBuild}=special?{data:{packageVersion:'not on npm (submodule showcase)'}}:await json(packageEvidence);
 if(!special&&!packageBuild.validation?.ok)throw Error(`${name}: package validation missing`);
 libraries.push({name,title,scope,upstream,homepage,evidence,evidenceSha256,packageEvidence,packageVersion:special?packageBuild.packageVersion:packageBuild.version,measuredAt,compilerSha256,timingScope,objectives});
 console.log(`Verified published measurements: ${name}`);
}
const result={schemaVersion:1,verifiedAt:new Date().toISOString(),method:'Each objective selects its own compiled artifact and the strongest recorded original baseline. All public artifact hashes were verified.',libraries};
mkdirSync(resolve(root,'src'),{recursive:true});mkdirSync(resolve(root,'public'),{recursive:true});
const text=JSON.stringify(result,null,2)+'\n';writeFileSync(resolve(root,'src/library-releases.json'),text);writeFileSync(resolve(root,'public/library-releases.json'),text);
