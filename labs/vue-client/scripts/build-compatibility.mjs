import {readFile, mkdir, writeFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {buildProfiles} from './compiler-delivery.mjs';
import {writePackageMetadata} from './package-metadata.mjs';
import {projectRoot} from '../tooling/compiler-path.mjs';

// Implementation instances are shared within each configured profile. Public
// entrypoints are compiler outputs forwarding those exact bindings. Declarations
// and package JSON remain packaging data; no emitted JavaScript is edited.
const implementations = [
  'development', 'test', 'compat', 'production', 'compat-production',
  'browser', 'browser-production', 'browser-compat', 'browser-compat-production',
  'cjs-development', 'cjs-production', 'cjs-compat', 'cjs-compat-production',
  'global', 'global-production', 'global-compat', 'global-compat-production',
];
const entries=JSON.parse(await readFile(resolve(projectRoot,'config/package-delivery.json'),'utf8'));
const profiles=process.env.VUELIL_PROFILES?.split(',').filter(Boolean) ?? [...implementations,...entries.profiles];
const reports=await buildProfiles(profiles);
const declarations=writePackageMetadata();
const report={schema:1, producer:'lilscript', profiles:reports, declarations,
  qualification:'Delivery receipts. Runtime, compression and release qualification are recorded separately.'};
await mkdir(resolve(projectRoot,'artifacts'),{recursive:true});
await writeFile(resolve(projectRoot,'artifacts/compiler-delivery-report.json'),JSON.stringify(report,null,2)+'\n');
console.log(JSON.stringify({profiles:reports.length, artifacts:reports.reduce((sum,row)=>sum+Object.keys(row.artifacts).length,0), declarations:declarations.length}));
