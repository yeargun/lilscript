# 040 — the shape, before and after the fusion

Excerpts from two scored candidates of the traced jquerylil compile (2026-09-01, 037 binary).

```js
// before: candidate 000030694-a038613f6297c822.js
t){var n=B[e];n=n||[];n=Nn(n,t,e,r);if(n)return n;return Nn(B["*"],t,e,r)}function


// after fold_assigned_truthy_ternaries had run on a sibling candidate: 000030282-1bc2d68f2d2e330b.js
Rn=(a,R,P)=>{var n=H[R];n=n||[],returnHn(n,P,R,a)||((n,P,j,F)=>{if(!n)return null;for(var a,A=n.length,R=0;R<A;R++)if(a=n[R].call(P,j,F))return a;return n

// as shipped (dist/jquery.raw.js, working tree):
Fn=(r,e,t)=>{var n=J[e];n=n||[],returnRn(n,t,e,r)||((e,t,n,r)=>{if(!e)return null;for(var i,l=e.length,a=0;a<l;
```

The trace filter (`finer/out/040/trace-filter.awk`) named fold #471 of 45067, `boolean::fold_assigned_truthy_ternaries`, as the first whose output carried `return[A-Z]…(`.
