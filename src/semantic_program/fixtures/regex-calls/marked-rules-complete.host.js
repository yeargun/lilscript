const {createHash}=await import('node:crypto');
const names=Object.keys(library).sort();
const rows=names.filter(name=>library[name] instanceof RegExp).map(name=>[name,library[name].source,library[name].flags]);
events.push(['exports',names.length,rows.length,names.filter(name=>typeof library[name]==='function')]);
events.push(['table-sha256',createHash('sha256').update(JSON.stringify(rows)).digest('hex')]);
const families=[['nextBulletRegex','nextBullet'],['hrIndentRegex','hrIndent'],['fencesBeginRegex','fencesBegin'],['headingBeginRegex','headingBegin'],['htmlBeginRegex','htmlBegin'],['blockquoteBeginRegex','blockquoteBegin']];
for(const [factory,prefix] of families){
 const selections=[];
 for(const indent of [-5,0,1,2,3,4,99,-2147483648,2147483647]){
  const selected=library[factory](indent);
  selections.push([0,1,2,3].find(index=>selected===library[prefix+index]));
 }
 events.push(['factory',factory,library[factory].length,selections]);
}
const a=library.listItemRegex('[*+-]'),b=library.listItemRegex('[*+-]');
a.lastIndex=77;
const matched=b.exec('- item\n');
events.push(['dynamic-factory',library.listItemRegex.length,a!==b,a.lastIndex,b.lastIndex,matched&&Array.from(matched)]);
const cases=[
 ['heading','## Title\n'],['hr','---\n'],['newline',' \n\n'],
 ['otherBlankLine',' \t'],['otherBlankLine','text'],
 ['otherListIsTask','[x] task'],['otherListIsTask','[ ] '],
 ['otherUnicodeAlphaNumeric','π'],['otherUnicodeAlphaNumeric','!']
];
for(const [name,text] of cases){library[name].lastIndex=0;events.push(['match',name,text,library[name].test(text)]);}
const tabs=library.otherTabCharGlobal;
tabs.lastIndex=0;
events.push(['global-state',tabs.test('\tA\t'),tabs.lastIndex,tabs.test('\tA\t'),tabs.lastIndex,tabs.test('\tA\t'),tabs.lastIndex]);
