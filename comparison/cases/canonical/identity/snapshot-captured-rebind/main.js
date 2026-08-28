let node = {__proto__: null, href: 42, title: 43};
const saved = node.href ?? 0;
const rebind = () => { node = {__proto__: null, href: 47, title: 0}; };
rebind();
console.log(saved + (node.href ?? 0) | 0);
