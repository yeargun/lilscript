function b(a){return a<=1?1:a*b(a-1|0)|0}console.log(b(7));for(var c=1071,d=462;d!==0;){let a=c%d|0;c=d;d=a}console.log(c);let e=0,f=1;for(let a=0;a<12;a=a+1|0){let g=e+f|0;e=f;f=g}console.log(e);
