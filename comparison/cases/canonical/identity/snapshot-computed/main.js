const node = {__proto__: null, href: 42, title: 43};
const saved = node["href"] ?? 0;
node.href = 33;
console.log(saved + (node.href ?? 0) | 0);
