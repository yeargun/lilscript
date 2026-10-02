const print=x=>console.log(x);
function resolved(value){return Promise.resolve(value);}async function returned(value){return value;}
async function adopted(){return Promise.resolve(23);}async function choice(direct){return direct?31:Promise.resolve(32);}
let selected=n=>'old '+n;async function change(){selected=n=>'new '+n;await Promise.resolve(0);return 4;}async function prepared(){return selected(await change());}
async function noop(){await Promise.resolve(0);}
async function cycles(){let left=null,right=null;left=Promise.resolve(0).then(n=>right??Promise.resolve(1));right=Promise.resolve(0).then(n=>left??Promise.resolve(2));await Promise.resolve(0);await Promise.resolve(0);print('cycles abandoned');}
async function run(){
 const base=Promise.resolve(21),same=Promise.resolve(base);print(base===same);print(await same);const canonical=base;print(await canonical);
 const generic=resolved(base);print(base===generic);print(await generic);print(await returned(base));print(await adopted());print(await choice(true));print(await choice(false));
 const variant=base,union=Promise.resolve(variant);print(await union);await noop().then(()=>print('void then'));print(await prepared());print(selected(4));
 let self=null;self=Promise.resolve(0).then(n=>self??Promise.resolve(0));try{print(await self);}catch(error){print(error.name);}self=null;
 await cycles();for(let i=0;i<100;i++){const failed=Promise.reject('handled');try{print(await failed);}catch{}}print('contracts done');
}
run();
