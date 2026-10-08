import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {pathToFileURL} from 'node:url';
import {writeFileSync} from 'node:fs';
const require=createRequire(process.argv[2]+'/package.json');
const {commonmark}=await import(pathToFileURL(require.resolve('commonmark.json')));
const {fromMarkdown:upstream}=await import(pathToFileURL(require.resolve('mdast-util-from-markdown')));
const {fromMarkdown:actual}=await import(pathToFileURL(process.argv[3]));
const failures=[];
for(const example of commonmark){try{assert.deepStrictEqual(actual(example.markdown),upstream(example.markdown));}catch(error){failures.push({example:example.example,section:example.section,message:error.message});}}
const result={tests:commonmark.length,passed:commonmark.length-failures.length,failures};
writeFileSync(process.argv[4],JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify({tests:result.tests,passed:result.passed,failed:failures.length,first:failures.slice(0,2)}));
process.exitCode=failures.length?1:0;
