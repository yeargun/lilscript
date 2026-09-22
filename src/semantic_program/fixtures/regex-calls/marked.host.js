events.push(['arity',library.replaceRe.length,library.execRe.length,library.expandTabs.length,library.removeCodeIndent.length]);
for(const [text,indent] of [['a\tb',0],['\t',0],['\t',2],['ab\t\tz',1],['😀\tX',0],['\0\ud800\tZ',0],['',3]]){
  const value=library.expandTabs(text,indent);
  events.push(['tabs',Array.from({length:value.length},(_,i)=>value.charCodeAt(i))]);
}
const remove=library.otherCodeRemoveIndent;
remove.lastIndex=99;
events.push(['indent',library.removeCodeIndent('    one\n\tsecond\n  third\nunindented'),remove.lastIndex]);
events.push(['indent-again',library.removeCodeIndent(' x\n    y'),remove.lastIndex]);
const tabs=library.otherTabCharGlobal;
tabs.lastIndex=0;
for(let i=0;i<3;i++){
 const match=library.execRe(tabs,'\tA\t');
 events.push(['exec',match===null?null:[match[0],match.index],tabs.lastIndex]);
}
events.push(['test',library.testRe(tabs,'\tA'),tabs.lastIndex,library.testRe(tabs,'\tA'),tabs.lastIndex]);
events.push(['replace',library.replaceRe('\ta\tb',tabs,'-'),tabs.lastIndex]);
events.push(['blank',library.testRe(library.otherBlankLine,' \t'),library.testRe(library.otherBlankLine,'x')]);
