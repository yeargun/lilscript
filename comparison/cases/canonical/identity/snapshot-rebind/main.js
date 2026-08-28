let node = {__proto__: null, href: 42, title: 43};
const next = {__proto__: null, href: 47, title: 0};
const saved = node.href ?? 0;
node = next;
console.log(saved + (node.href ?? 0) | 0);
