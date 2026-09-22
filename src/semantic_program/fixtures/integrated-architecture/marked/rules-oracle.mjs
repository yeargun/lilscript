import {readFileSync,writeFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {Lexer} from '/home/azureuser/markedlil/node_modules/marked/lib/marked.esm.js';
const sourcePath='/home/azureuser/markedlil/src/rules.lil';
const upstreamPath='/home/azureuser/markedlil/node_modules/marked/lib/marked.esm.js';
const source=readFileSync(sourcePath,'utf8');
const {block:b,inline:i}=Lexer.rules;
const other=new Lexer().tokenizer.rules.other;
const named={
 newline:b.normal.newline,blockCode:b.normal.code,fences:b.normal.fences,hr:b.normal.hr,heading:b.normal.heading,lheading:b.normal.lheading,lheadingGfm:b.gfm.lheading,def:b.normal.def,list:b.normal.list,html:b.normal.html,paragraph:b.normal.paragraph,blockquote:b.normal.blockquote,gfmTable:b.gfm.table,paragraphGfm:b.gfm.paragraph,blockText:b.normal.text,htmlPed:b.pedantic.html,defPed:b.pedantic.def,headingPed:b.pedantic.heading,lheadingPed:b.pedantic.lheading,paragraphPed:b.pedantic.paragraph,
 escapeRe:i.normal.escape,inlineCode:i.normal.code,br:i.normal.br,inlineText:i.normal.text,punctuation:i.normal.punctuation,blockSkip:i.normal.blockSkip,emStrongLDelim:i.normal.emStrongLDelim,emStrongLDelimGfm:i.gfm.emStrongLDelim,emStrongLDelimPedantic:i.pedantic.emStrongLDelim,emStrongRDelimAst:i.normal.emStrongRDelimAst,emStrongRDelimAstGfm:i.gfm.emStrongRDelimAst,emStrongRDelimAstPedantic:i.pedantic.emStrongRDelimAst,emStrongRDelimUnd:i.normal.emStrongRDelimUnd,emStrongRDelimUndPedantic:i.pedantic.emStrongRDelimUnd,delLDelim:i.gfm.delLDelim,delRDelim:i.gfm.delRDelim,anyPunctuation:i.normal.anyPunctuation,autolink:i.normal.autolink,tag:i.normal.tag,link:i.normal.link,reflink:i.normal.reflink,nolink:i.normal.nolink,reflinkSearch:i.normal.reflinkSearch,urlRe:i.gfm.url,backpedal:i.gfm._backpedal,delRe:i.gfm.del,textGfm:i.gfm.text,brBreaks:i.breaks.br,textBreaks:i.breaks.text,linkPed:i.pedantic.link,reflinkPed:i.pedantic.reflink,
};
for(const [key,value] of Object.entries(other))if(value instanceof RegExp)named['other'+key[0].toUpperCase()+key.slice(1)]=value;
for(const [name,factory] of [['nextBullet',other.nextBulletRegex],['hrIndent',other.hrRegex],['fencesBegin',other.fencesBeginRegex],['headingBegin',other.headingBeginRegex],['htmlBegin',other.htmlBeginRegex],['blockquoteBegin',other.blockquoteBeginRegex]])for(let n=0;n<4;n++)named[name+n]=factory(n+1);
// Decode only the two escapes emitted by generate-rules.mjs lilString.
// This is an independent narrow archival check, not the compiler lexer.
function decode(s){let out='';for(let p=0;p<s.length;p++){if(s[p]!=='\\'){out+=s[p];continue;}const c=s[++p];if(c!=='\\'&&c!=='"')throw Error('unrecognized generator escape');out+=c;}return out;}
const table=[];const mismatches=[];const spellingAdjustments=[];
for(const line of source.split('\n')){if(!line.startsWith('export Regex ')||!line.includes(' = new Regex('))continue;
 const m=/^export Regex (\w+) = new Regex\("((?:\\.|[^"\\])*)"(?:, "((?:\\.|[^"\\])*)")?\);$/.exec(line);
 if(!m)throw Error('unrecognized declaration '+line.slice(0,80));
 const [_,name,pattern,flags]=m;
 let upstream=named[name];if(!(upstream instanceof RegExp))throw Error('missing upstream Regex '+name);
 if(/^nextBullet[0-3]$/.test(name)){const adjusted=upstream.source.replaceAll('\t','\\t');spellingAdjustments.push({name,upstreamSource:upstream.source,archivedSource:adjusted,reason:'The maintained generator uses escaped tab in this character class; upstream uses a literal tab. Preserve the library source spelling.'});upstream=new RegExp(adjusted,upstream.flags);}
 const local=new RegExp(decode(pattern),decode(flags??''));
 if(local.source!==upstream.source||local.flags!==upstream.flags)mismatches.push({name,local:[local.source,local.flags],upstream:[upstream.source,upstream.flags]});
 table.push([name,upstream.source,upstream.flags]);
}
table.sort((a,b)=>a[0]<b[0]?-1:a[0]>b[0]?1:0);
const hash=x=>createHash('sha256').update(x).digest('hex');
writeFileSync('/tmp/lilscript-regex-calls-20260913/rules-oracle-mismatches.json',JSON.stringify(mismatches,null,2)+'\n');
if(mismatches.length)throw Error(mismatches.length+' upstream mismatches; see report');
if(table.length!==117)throw Error('unexpected table length '+table.length);
writeFileSync('/tmp/lilscript-regex-calls-20260913/rules-oracle-table.json',JSON.stringify(table)+'\n');
const provenance={source:'Marked 18.0.10 installed upstream Lexer rules and tokenizer.rules.other; no generator execution',sourcePath,sourceSha256:hash(source),upstreamPath,upstreamSha256:hash(readFileSync(upstreamPath)),oracleScriptSha256:hash(readFileSync(import.meta.filename)),entries:table.length,exactUpstreamEntries:table.length-spellingAdjustments.length,spellingAdjustments,tableSerialization:'JSON.stringify(sorted array of [export name, RegExp.source, RegExp.flags]), with documented upstream tab spelling adjustments and no final newline',tableSha256:hash(JSON.stringify(table)),matchingArchivedDeclarations:table.length,node:process.version};
writeFileSync('/tmp/lilscript-regex-calls-20260913/rules-oracle-origin.json',JSON.stringify(provenance,null,2)+'\n');
console.log(JSON.stringify(provenance,null,2));
