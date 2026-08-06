let s=0,o=0;for(let i=0;i<180000;i++){let v=i%97;s=Math.imul(s,31)+v|0;o=o+v+6|0}console.log(`events:${s}:${o}`)
