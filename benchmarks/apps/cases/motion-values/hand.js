let p=0,h=0,s=0;for(let i=0;i<160000;i++){let l=i%8;p+=l*60-120;h+=i*47%360;s+=l+2}console.log(`motion:${p}:${h}:${s}:5494928`)
