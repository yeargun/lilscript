import {readFileSync,writeFileSync,existsSync} from 'node:fs';
import {createServer} from 'node:http';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import os from 'node:os';
import {pathToFileURL} from 'node:url';
const {chromium}=await import(process.env.PLAYWRIGHT_MODULE??'playwright');
const run=resolve(process.argv[2]??'site/performance'),root=existsSync(join(run,'motion-benchmark'))?join(run,'motion-benchmark'):run;
const server=createServer((req,res)=>{try{const file=resolve(root,'.'+decodeURIComponent(new URL(req.url,'http://localhost').pathname).replace(/\/$/,'/index.html'));if(!file.startsWith(root+'/')){res.writeHead(403);res.end();return}res.setHeader('Content-Type',file.endsWith('.html')?'text/html':file.endsWith('.json')?'application/json':'text/javascript');res.end(readFileSync(file))}catch{res.writeHead(404);res.end()}});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const baseUrl='http://127.0.0.1:'+server.address().port+'/';
const smoke=process.argv.includes('--smoke'), repetitions=smoke?1:30,warmups=smoke?0:2;
const workloads=JSON.parse(readFileSync(join(root,'workloads.json'),'utf8'));
const only=process.argv.includes('--workloads')?process.argv[process.argv.indexOf('--workloads')+1].split(','):null;
const median=xs=>{const a=[...xs].sort((a,b)=>a-b);return a.length%2?a[(a.length-1)/2]:(a[a.length/2-1]+a[a.length/2])/2};
const quantile=(xs,p)=>[...xs].sort((a,b)=>a-b)[Math.min(xs.length-1,Math.floor(xs.length*p))];
const hash=p=>createHash('sha256').update(readFileSync(p)).digest('hex');
const browser=await chromium.launch({headless:true,args:['--disable-background-timer-throttling','--disable-renderer-backgrounding','--disable-backgrounding-occluded-windows']});
const result={schemaVersion:1,measuredAt:new Date().toISOString(),browser:browser.version(),machine:{cpu:os.cpus()[0].model,logicalCpus:os.cpus().length,memoryBytes:os.totalmem(),os:execFileSync('lsb_release',['-ds'],{encoding:'utf8'}).trim(),node:process.version,architecture:os.arch(),loadAtStart:os.loadavg()},protocol:{repetitions,warmups,order:'Alternating original/LilScript and LilScript/original for each paired repetition; fresh browser page per lane.',viewport:{width:1280,height:900},deviceScaleFactor:1,durationMs:600,observationMs:800,primary:'Renderer main-thread TaskDuration from Chromium DevTools Performance counters; fixed identical animation work.',excluded:'Bundle download, module evaluation and initial DOM creation occur before the CPU interval.',validation:'Five paused timeline samples plus final DOM values; identical element counts, keyframes, easing, durations, styles and viewport.',equivalenceMargin:0.05,confidence:'95% paired bootstrap interval of median CPU ratio; 10,000 resamples. Equivalence requires the entire interval within [0.95, 1.05].',headless:true,frameCaveat:'Frame cadence is separately reported; equal configured duration or display-limited FPS alone does not establish equal runtime cost.'},inputs:{lilscript:{file:'inputs/lilscript.js',sha256:hash(join(root,'inputs/lilscript.js'))},original:{file:'inputs/original.js',sha256:hash(join(root,'inputs/original.js'))},harnessSha256:hash(join(root,'harness.mjs'))},workloads:[]};
const save=()=>writeFileSync(join(root,smoke?'smoke.json':'performance.json'),JSON.stringify(result,null,2)+'\n');
async function trial(lane,workload,kind){
 const page=await browser.newPage({viewport:result.protocol.viewport,deviceScaleFactor:1,reducedMotion:'no-preference'});const errors=[];page.on('pageerror',e=>errors.push(String(e)));
 try{
  await page.goto(baseUrl,{waitUntil:'load'});await page.waitForFunction(()=>!!window.motionMeasurement);
  await page.evaluate(({lane,id})=>window.motionMeasurement.prepare(lane,id),{lane,id:workload.id});
  if(kind==='validate'){const samples=await page.evaluate(()=>window.motionMeasurement.validate());return{samples,errors};}
  const session=await page.context().newCDPSession(page);await session.send('HeapProfiler.collectGarbage');await session.send('Performance.enable');
  const before=Object.fromEntries((await session.send('Performance.getMetrics')).metrics.map(x=>[x.name,x.value]));
  const measured=await page.evaluate(()=>window.motionMeasurement.measure());
  const after=Object.fromEntries((await session.send('Performance.getMetrics')).metrics.map(x=>[x.name,x.value]));
  const cpu={};for(const key of ['TaskDuration','ScriptDuration','LayoutDuration','RecalcStyleDuration'])cpu[key]=(after[key]-before[key])*1000;
  return {...measured,cpu,errors};
 }catch(e){return{errors:[...errors,String(e)]}}finally{await page.close()}
}
function compare(a,b){
 const errors=[];if(!a||!b||a.length!==b.length)return['Missing paired values'];
 for(let i=0;i<a.length;i++)for(const key of ['opacity','x','y','width','marginRight']){const tolerance=key==='opacity'?.02:.6;if(!Number.isFinite(a[i][key])||!Number.isFinite(b[i][key])||Math.abs(a[i][key]-b[i][key])>tolerance){errors.push(`element ${i} ${key}: LilScript ${a[i][key]}, original ${b[i][key]}`);if(errors.length===6)return errors}}
 return errors;
}
let seed=0x1a2b3c4d;function random(){seed^=seed<<13;seed^=seed>>>17;seed^=seed<<5;return(seed>>>0)/4294967296}
function interval(ratios){const values=[];for(let i=0;i<10000;i++)values.push(median(ratios.map(()=>ratios[Math.floor(random()*ratios.length)])));return[quantile(values,.025),quantile(values,.975)]}
for(const workload of workloads){
 if(only&&!only.includes(workload.id))continue;
 const row={...workload,validation:{},samples:[],status:'pending'};result.workloads.push(row);save();
 for(const lane of ['original','lilscript'])row.validation[lane]=await trial(lane,workload,'validate');
 const errors=[...row.validation.original.errors,...row.validation.lilscript.errors];
 if(!errors.length)for(let i=0;i<5;i++)errors.push(...compare(row.validation.lilscript.samples[i]?.values,row.validation.original.samples[i]?.values).map(x=>`${i/4}: ${x}`));
 row.validation.errors=errors;
 if(errors.length){row.status='behavior-mismatch';console.log(workload.id,'BEHAVIOR MISMATCH',errors.slice(0,2).join('; '));save();continue;}
 let finalReference;
 for(let repetition=-warmups;repetition<repetitions;repetition++){
  const pair={repetition};for(const lane of ((repetition+warmups)%2===0?['original','lilscript']:['lilscript','original']))pair[lane]=await trial(lane,workload,'measure');
  const failures=[...pair.original.errors,...pair.lilscript.errors,...compare(pair.lilscript.final,pair.original.final)];
  if(failures.length){row.status='behavior-mismatch';row.validation.errors.push(...failures);break;}
  if(repetition>=0)row.samples.push(pair);save();
 }
 if(row.status==='behavior-mismatch'){console.log(workload.id,'MEASUREMENT FAILED');continue;}
 const summary={};for(const lane of ['original','lilscript']){
  const samples=row.samples.map(x=>x[lane]);summary[lane]={mainThreadMs:median(samples.map(x=>x.cpu.TaskDuration)),scriptMs:median(samples.map(x=>x.cpu.ScriptDuration)),styleLayoutMs:median(samples.map(x=>x.cpu.LayoutDuration+x.cpu.RecalcStyleDuration)),setupMs:median(samples.map(x=>x.setupMs)),frameP95Ms:median(samples.map(x=>quantile(x.frameIntervals,.95))),medianFrameMs:median(samples.map(x=>median(x.frameIntervals))),frames:median(samples.map(x=>x.frameIntervals.length))};
 }
 const ratios=row.samples.map(x=>x.lilscript.cpu.TaskDuration/x.original.cpu.TaskDuration);summary.cpuRatio=median(ratios);summary.cpuRatio95=interval(ratios);summary.withinFivePercent=summary.cpuRatio95[0]>=.95&&summary.cpuRatio95[1]<=1.05;row.summary=summary;row.status='measured';save();console.log(workload.id,summary.cpuRatio.toFixed(3)+'×',summary.cpuRatio95.map(x=>x.toFixed(3)).join('–'));
}
result.machine.loadAtEnd=os.loadavg();result.finishedAt=new Date().toISOString();save();await browser.close();await new Promise(resolve=>server.close(resolve));
