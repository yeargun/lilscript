const b=[1,2,3,4,5,6].map(a=>a*3|0).filter(a=>(a%2|0)===0),d=b.reduce((a,c)=>a+c|0,0);b.forEach(a=>{console.log(a)});console.log(d);
