let c=0;for(let a=0;a<12;a=a+1|0){if((a%3|0)===0)continue;let b=0;for(;b<4;)c=((a+b|0)%2|0)===0?c+Math.imul(a,b)|0:c+1|0,b=b+1|0}console.log(c);
