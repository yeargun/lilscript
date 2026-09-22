import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
const { chromium } = await import(pathToFileURL('/home/azureuser/katexlil/node_modules/playwright/index.mjs'));
const run=resolve(process.argv[2]);
const publications=JSON.parse(readFileSync(run+'/publications.json','utf8'));
const browser=await chromium.launch({headless:true});
const results=[];
for (const [name,value] of Object.entries(publications)) {
  if (process.argv.includes('--ports') && !process.argv[process.argv.indexOf('--ports')+1].split(',').includes(name)) continue;
  if (!value.commit) continue;
  const page=await browser.newPage({viewport:{width:1440,height:1000}});
  const errors=[];
  page.on('pageerror',error=>errors.push(String(error)));
  const url=`http://127.0.0.1:8844/publish/${name}/${name==='vuelil'?'web':'_site'}/`;
  try {
    await page.goto(url,{waitUntil:'domcontentloaded',timeout:15000});
    await page.waitForTimeout(1200);
    const panel=await page.locator('#build-comparison').count();
    const machine=panel?await page.locator('#build-comparison').innerText():'';
    if (panel!==1 || !machine.includes('Standard_D16als_v7')) errors.push('Missing build-machine methodology note');
    if (await page.locator('#build-audit, .compiler-progress, [data-audit-history]').count()) errors.push('Public page includes audit or compiler history');
    const response=await page.request.get(url+'comparison.json');
    const comparison=await response.json();
    if (!comparison.esm.original?.brotli11 || comparison.build.originalSeconds==null) errors.push('Missing original ESM comparison');
    if (comparison.buildComplete && !comparison.esm.lilscript?.brotli11) errors.push('Missing LilScript ESM comparison');
    const headline=await page.locator('.score, .scorecard, .hero').first().innerText().catch(()=>'');
    results.push({name,url,errors,headline});
    if (['rehype-katexlil','motionlil','monacolil','solidlil','zodlil','vuelil'].includes(name)) {
      await page.screenshot({path:run+'/preview-'+name+'.png',fullPage:true});
      await page.setViewportSize({width:390,height:844});
      await page.screenshot({path:run+'/preview-'+name+'-mobile.png',fullPage:true});
      results.at(-1).mobileOverflow=await page.evaluate(()=>Math.max(0,document.documentElement.scrollWidth-innerWidth));
    }
  } catch(error) { results.push({name,url,errors:[...errors,String(error)]}); }
  await page.close();
}
await browser.close();
writeFileSync(run+'/browser-checks.json',JSON.stringify(results,null,2)+'\n');
for (const result of results) console.log(result.name,result.errors.length?'FAILED '+result.errors.join(' | '):'OK');
if (results.some(result=>result.errors.length)) process.exitCode=1;
