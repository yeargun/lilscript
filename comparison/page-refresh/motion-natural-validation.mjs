import {readFileSync,writeFileSync} from 'node:fs';
import {createServer} from 'node:http';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
const {chromium}=await import(process.env.PLAYWRIGHT_MODULE??'playwright');
const root=resolve(process.argv[2]);
const server=createServer((req,res)=>{try{const path=resolve(root,'.'+new URL(req.url,'http://localhost').pathname.replace(/\/$/,'/index.html'));if(!path.startsWith(root+'/'))throw Error();res.setHeader('Content-Type',path.endsWith('.html')?'text/html':'text/javascript');res.end(readFileSync(path))}catch{res.writeHead(404);res.end()}});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const b=await chromium.launch({headless:true,args:['--disable-background-timer-throttling','--disable-renderer-backgrounding','--disable-backgrounding-occluded-windows']});
const metrics=JSON.parse(readFileSync(join(root,'performance.json'))),results=[];
for(const work of metrics.workloads.filter(x=>x.status==='measured')){
 const row={id:work.id,lanes:{}};
 for(const lane of ['original','lilscript']){
  row.lanes[lane]=[];
  for(let i=0;i<3;i++){
   const p=await b.newPage({viewport:{width:1280,height:900},deviceScaleFactor:1});await p.goto('http://127.0.0.1:'+server.address().port+'/');
   await p.evaluate(({lane,id})=>window.motionMeasurement.prepare(lane,id),{lane,id:work.id});
   const sampled=await p.evaluate(async()=>{
    const frames=[],start=performance.now(),running=window.motionMeasurement.measure();
    await new Promise(resolve=>{function frame(now){const s=getComputedStyle(document.querySelector('.dot'));frames.push({ms:now-start,opacity:+s.opacity});if(now-start<720)requestAnimationFrame(frame);else resolve()}requestAnimationFrame(frame)});
    await running;return frames;
   });
   const active=sampled.filter(x=>x.ms>60&&x.ms<560),changes=active.filter((x,i)=>i&&Math.abs(x.opacity-active[i-1].opacity)>.003).length;
   const error=Math.max(...active.map(x=>Math.abs((x.opacity-.25)/.75-x.ms/600)));
   row.lanes[lane].push({samples:sampled,activeFrames:active.length,changedFrames:changes,maxProgressError:error});await p.close();
  }
 }
 const minimumUpdateFraction=Math.min(...row.lanes.lilscript.map(x=>x.changedFrames/Math.max(1,x.activeFrames-1)));
 row.matches=minimumUpdateFraction>=.9&&Object.values(row.lanes).flat().every(x=>x.maxProgressError<.08);
 row.minimumUpdateFraction=minimumUpdateFraction;results.push(row);console.log(row.id,row.matches?'MATCH':'MISMATCH',minimumUpdateFraction);
}
const result={measuredAt:new Date().toISOString(),browser:b.version(),protocol:'Separate untimed natural playback check; three fresh pages per lane; read the first element opacity each animation frame. Linear timeline deviation must be <8% and LilScript must visibly advance on at least 90% of active frames. These intrusive style reads are not part of CPU timings.',harnessSha256:createHash('sha256').update(readFileSync(join(root,'harness.mjs'))).digest('hex'),results};
writeFileSync(join(root,'natural-validation.json'),JSON.stringify(result,null,2)+'\n');await b.close();await new Promise(resolve=>server.close(resolve));
if(results.some(x=>!x.matches))process.exitCode=1;
