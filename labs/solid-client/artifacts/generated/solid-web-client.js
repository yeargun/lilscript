//#region node_modules/solid-js/dist/solid.js
var e = {
	context: void 0,
	registry: void 0,
	effects: void 0,
	done: !1,
	getContextId() {
		return t(this.context.count);
	},
	getNextContextId() {
		return t(this.context.count++);
	}
};
function t(t) {
	let n = String(t), r = n.length - 1;
	return e.context.id + (r ? String.fromCharCode(96 + r) : "") + n;
}
function n(t) {
	e.context = t;
}
var r = (e, t) => e === t, i = Symbol("solid-proxy"), a = typeof Proxy == "function", o = Symbol("solid-track"), s = { equals: r }, c = null, l = de, u = 1, d = 2, f = {
	owned: null,
	cleanups: null,
	context: null,
	owner: null
}, p = null, m = null, h = null, g = null, _ = null, v = 0;
function y(e, t) {
	let n = h, r = p, i = e.length === 0, a = t === void 0 ? r : t, o = i ? f : {
		owned: null,
		cleanups: null,
		context: a ? a.context : null,
		owner: a
	}, s = i ? e : () => e(() => C(() => F(o)));
	p = o, h = null;
	try {
		return N(s, !0);
	} finally {
		h = n, p = r;
	}
}
function b(e, t) {
	t = t ? Object.assign({}, s, t) : s;
	let n = {
		value: e,
		observers: null,
		observerSlots: null,
		comparator: t.equals || void 0
	};
	return [O.bind(n), (e) => (typeof e == "function" && (e = m && m.running && m.sources.has(n) ? e(n.tValue) : e(n.value)), k(n, e))];
}
function x(e, t, n) {
	A(j(e, t, !1, u));
}
function ee(e, t, n) {
	l = fe;
	let r = j(e, t, !1, u), i = D && E(D);
	i && (r.suspense = i), (!n || !n.render) && (r.user = !0), _ ? _.push(r) : A(r);
}
function S(e, t, n) {
	n = n ? Object.assign({}, s, n) : s;
	let r = j(e, t, !0, 0);
	return r.observers = null, r.observerSlots = null, r.comparator = n.equals || void 0, A(r), O.bind(r);
}
function C(e) {
	if (h === null) return e();
	let t = h;
	h = null;
	try {
		return e();
	} finally {
		h = t;
	}
}
function w(e) {
	return p === null || (p.cleanups === null ? p.cleanups = [e] : p.cleanups.push(e)), e;
}
function te(e, t) {
	c ||= Symbol("error"), p = j(void 0, void 0, !0), p.context = {
		...p.context,
		[c]: [t]
	}, m && m.running && m.sources.add(p);
	try {
		return e();
	} catch (e) {
		I(e);
	} finally {
		p = p.owner;
	}
}
function T() {
	return p;
}
function ne(e, t) {
	let n = p, r = h;
	p = e, h = null;
	try {
		return N(t, !0);
	} catch (e) {
		I(e);
	} finally {
		p = n, h = r;
	}
}
var [re, ie] = /*@__PURE__*/ b(!1);
function ae(e) {
	_.push.apply(_, e), e.length = 0;
}
function oe(e, t) {
	let n = Symbol("context");
	return {
		id: n,
		Provider: _e(n),
		defaultValue: e
	};
}
function E(e) {
	let t;
	return p && p.context && (t = p.context[e.id]) !== void 0 ? t : e.defaultValue;
}
function se(e) {
	let t = S(e), n = S(() => L(t()));
	return n.toArray = () => {
		let e = n();
		return Array.isArray(e) ? e : e == null ? [] : [e];
	}, n;
}
var D;
function ce() {
	return D ||= oe();
}
function O() {
	let e = m && m.running;
	if (this.sources && (e ? this.tState : this.state)) {
		if ((e ? this.tState : this.state) === u) A(this);
		else {
			let e = g;
			g = null, N(() => P(this), !1), g = e;
		}
	}
	if (h) {
		let e = this.observers;
		if (!e || e[e.length - 1] !== h) {
			let t = e ? e.length : 0;
			h.sources ? (h.sources.push(this), h.sourceSlots.push(t)) : (h.sources = [this], h.sourceSlots = [t]), e ? (e.push(h), this.observerSlots.push(h.sources.length - 1)) : (this.observers = [h], this.observerSlots = [h.sources.length - 1]);
		}
	}
	return e && m.sources.has(this) ? this.tValue : this.value;
}
function k(e, t, n) {
	let r = m && m.running && m.sources.has(e) ? e.tValue : e.value;
	if (!e.comparator || !e.comparator(r, t)) {
		if (m) {
			let r = m.running;
			(r || !n && m.sources.has(e)) && (m.sources.add(e), e.tValue = t), r || (e.value = t);
		} else e.value = t;
		e.observers && e.observers.length && N(() => {
			for (let t = 0; t < e.observers.length; t += 1) {
				let n = e.observers[t], r = m && m.running;
				r && m.disposed.has(n) || ((r ? !n.tState : !n.state) && (n.pure ? g.push(n) : _.push(n), n.observers && pe(n)), r ? n.tState = u : n.state = u);
			}
			if (g.length > 1e6) throw g = [], Error();
		}, !1);
	}
	return t;
}
function A(e) {
	if (!e.fn) return;
	F(e);
	let t = v;
	le(e, m && m.running && m.sources.has(e) ? e.tValue : e.value, t), m && !m.running && m.sources.has(e) && queueMicrotask(() => {
		N(() => {
			m && (m.running = !0), h = p = e, le(e, e.tValue, t), h = p = null;
		}, !1);
	});
}
function le(e, t, n) {
	let r, i = p, a = h;
	h = p = e;
	try {
		r = e.fn(t);
	} catch (t) {
		return e.pure && (m && m.running ? (e.tState = u, e.tOwned && e.tOwned.forEach(F), e.tOwned = void 0) : (e.state = u, e.owned && e.owned.forEach(F), e.owned = null)), e.updatedAt = n + 1, I(t);
	} finally {
		h = a, p = i;
	}
	(!e.updatedAt || e.updatedAt <= n) && (e.updatedAt != null && "observers" in e ? k(e, r, !0) : m && m.running && e.pure ? (m.sources.has(e) || (e.value = r), m.sources.add(e), e.tValue = r) : e.value = r, e.updatedAt = n);
}
function j(e, t, n, r = u, i) {
	let a = {
		fn: e,
		state: r,
		updatedAt: null,
		owned: null,
		sources: null,
		sourceSlots: null,
		cleanups: null,
		value: t,
		owner: p,
		context: p ? p.context : null,
		pure: n
	};
	return m && m.running && (a.state = 0, a.tState = r), p === null || p !== f && (m && m.running && p.pure ? p.tOwned ? p.tOwned.push(a) : p.tOwned = [a] : p.owned ? p.owned.push(a) : p.owned = [a]), a;
}
function M(e) {
	let t = m && m.running;
	if ((t ? e.tState : e.state) === 0) return;
	if ((t ? e.tState : e.state) === d) return P(e);
	if (e.suspense && C(e.suspense.inFallback)) return e.suspense.effects.push(e);
	let n = [e];
	for (; (e = e.owner) && (!e.updatedAt || e.updatedAt < v);) {
		if (t && m.disposed.has(e)) return;
		(t ? e.tState : e.state) && n.push(e);
	}
	for (let r = n.length - 1; r >= 0; r--) {
		if (e = n[r], t) {
			let t = e, i = n[r + 1];
			for (; (t = t.owner) && t !== i;) if (m.disposed.has(t)) return;
		}
		if ((t ? e.tState : e.state) === u) A(e);
		else if ((t ? e.tState : e.state) === d) {
			let t = g;
			g = null, N(() => P(e, n[0]), !1), g = t;
		}
	}
}
function N(e, t) {
	if (g) return e();
	let n = !1;
	t || (g = []), _ ? n = !0 : _ = [], v++;
	try {
		let t = e();
		return ue(n), t;
	} catch (e) {
		n || (_ = null), g = null, I(e);
	}
}
function ue(e) {
	if (g &&= (de(g), null), e) return;
	let t;
	if (m) {
		if (!m.promises.size && !m.queue.size) {
			let e = m.sources, n = m.disposed;
			_.push.apply(_, m.effects), t = m.resolve;
			for (let e of _) "tState" in e && (e.state = e.tState), delete e.tState;
			m = null, N(() => {
				for (let e of n) F(e);
				for (let t of e) {
					if (t.value = t.tValue, t.owned) for (let e = 0, n = t.owned.length; e < n; e++) F(t.owned[e]);
					t.tOwned && (t.owned = t.tOwned), delete t.tValue, delete t.tOwned, t.tState = 0;
				}
				ie(!1);
			}, !1);
		} else if (m.running) {
			m.running = !1, m.effects.push.apply(m.effects, _), _ = null, ie(!0);
			return;
		}
	}
	let n = _;
	_ = null, n.length && N(() => l(n), !1), t && t();
}
function de(e) {
	for (let t = 0; t < e.length; t++) M(e[t]);
}
function fe(t) {
	let r, i = 0;
	for (r = 0; r < t.length; r++) {
		let e = t[r];
		e.user ? t[i++] = e : M(e);
	}
	if (e.context) {
		if (e.count) {
			e.effects ||= [], e.effects.push(...t.slice(0, i));
			return;
		}
		n();
	}
	for (e.effects && (e.done || !e.count) && (t = [...e.effects, ...t], i += e.effects.length, delete e.effects), r = 0; r < i; r++) M(t[r]);
}
function P(e, t) {
	let n = m && m.running;
	n ? e.tState = 0 : e.state = 0;
	for (let r = 0; r < e.sources.length; r += 1) {
		let i = e.sources[r];
		if (i.sources) {
			let e = n ? i.tState : i.state;
			e === u ? i !== t && (!i.updatedAt || i.updatedAt < v) && M(i) : e === d && P(i, t);
		}
	}
}
function pe(e) {
	let t = m && m.running;
	for (let n = 0; n < e.observers.length; n += 1) {
		let r = e.observers[n];
		(t ? !r.tState : !r.state) && (t ? r.tState = d : r.state = d, r.pure ? g.push(r) : _.push(r), r.observers && pe(r));
	}
}
function F(e) {
	let t;
	if (e.sources) for (; e.sources.length;) {
		let t = e.sources.pop(), n = e.sourceSlots.pop(), r = t.observers;
		if (r && r.length) {
			let e = r.pop(), i = t.observerSlots.pop();
			n < r.length && (e.sourceSlots[i] = n, r[n] = e, t.observerSlots[n] = i);
		}
	}
	if (e.tOwned) {
		for (t = e.tOwned.length - 1; t >= 0; t--) F(e.tOwned[t]);
		delete e.tOwned;
	}
	if (m && m.running && e.pure) me(e, !0);
	else if (e.owned) {
		for (t = e.owned.length - 1; t >= 0; t--) F(e.owned[t]);
		e.owned = null;
	}
	if (e.cleanups) {
		for (t = e.cleanups.length - 1; t >= 0; t--) e.cleanups[t]();
		e.cleanups = null;
	}
	m && m.running ? e.tState = 0 : e.state = 0;
}
function me(e, t) {
	if (t || (e.tState = 0, m.disposed.add(e)), e.owned) for (let t = 0; t < e.owned.length; t++) me(e.owned[t]);
}
function he(e) {
	return e instanceof Error ? e : Error(typeof e == "string" ? e : "Unknown error", { cause: e });
}
function ge(e, t, n) {
	try {
		for (let n of t) n(e);
	} catch (e) {
		I(e, n && n.owner || null);
	}
}
function I(e, t = p) {
	let n = c && t && t.context && t.context[c], r = he(e);
	if (!n) throw r;
	_ ? _.push({
		fn() {
			ge(r, n, t);
		},
		state: u
	}) : ge(r, n, t);
}
function L(e) {
	if (typeof e == "function" && !e.length) return L(e());
	if (Array.isArray(e)) {
		let t = [];
		for (let n = 0; n < e.length; n++) {
			let r = L(e[n]);
			if (Array.isArray(r)) {
				if (r.length < 32768) t.push.apply(t, r);
				else for (let e = 0; e < r.length; e++) t.push(r[e]);
			} else t.push(r);
		}
		return t;
	}
	return e;
}
function _e(e, t) {
	return function(t) {
		let n;
		return x(() => n = C(() => (p.context = {
			...p.context,
			[e]: t.value
		}, se(() => t.children))), void 0), n;
	};
}
var R = Symbol("fallback");
function z(e) {
	for (let t = 0; t < e.length; t++) e[t]();
}
function ve(e, t, n = {}) {
	let r = [], i = [], a = [], s = 0, c = t.length > 1 ? [] : null;
	return w(() => z(a)), () => {
		let l = e() || [], u = l.length, d, f;
		return l[o], C(() => {
			let e, t, o, m, h, g, _, v, b;
			if (u === 0) s !== 0 && (z(a), a = [], r = [], i = [], s = 0, c &&= []), n.fallback && (r = [R], i[0] = y((e) => (a[0] = e, n.fallback())), s = 1);
			else if (s === 0) {
				for (i = Array(u), f = 0; f < u; f++) r[f] = l[f], i[f] = y(p);
				s = u;
			} else {
				for (o = Array(u), m = Array(u), c && (h = Array(u)), g = 0, _ = Math.min(s, u); g < _ && r[g] === l[g]; g++);
				for (_ = s - 1, v = u - 1; _ >= g && v >= g && r[_] === l[v]; _--, v--) o[v] = i[_], m[v] = a[_], c && (h[v] = c[_]);
				for (e = /* @__PURE__ */ new Map(), t = Array(v + 1), f = v; f >= g; f--) b = l[f], d = e.get(b), t[f] = d === void 0 ? -1 : d, e.set(b, f);
				for (d = g; d <= _; d++) b = r[d], f = e.get(b), f !== void 0 && f !== -1 ? (o[f] = i[d], m[f] = a[d], c && (h[f] = c[d]), f = t[f], e.set(b, f)) : a[d]();
				for (f = g; f < u; f++) f in o ? (i[f] = o[f], a[f] = m[f], c && (c[f] = h[f], c[f](f))) : i[f] = y(p);
				i = i.slice(0, s = u), r = l.slice(0);
			}
			return i;
		});
		function p(e) {
			if (a[f] = e, c) {
				let [e, n] = b(f);
				return c[f] = n, t(l[f], e);
			}
			return t(l[f]);
		}
	};
}
function ye(e, t, n = {}) {
	let r = [], i = [], a = [], s = [], c = 0, l;
	return w(() => z(a)), () => {
		let u = e() || [], d = u.length;
		return u[o], C(() => {
			if (d === 0) return c !== 0 && (z(a), a = [], r = [], i = [], c = 0, s = []), n.fallback && (r = [R], i[0] = y((e) => (a[0] = e, n.fallback())), c = 1), i;
			for (r[0] === R && (a[0](), a = [], r = [], i = [], c = 0), l = 0; l < d; l++) l < r.length && r[l] !== u[l] ? s[l](() => u[l]) : l >= r.length && (i[l] = y(f));
			for (; l < r.length; l++) a[l]();
			return c = s.length = a.length = d, r = u.slice(0), i = i.slice(0, c);
		});
		function f(e) {
			a[l] = e;
			let [n, r] = b(u[l]);
			return s[l] = r, t(n, l);
		}
	};
}
function B(e, t) {
	return C(() => e(t || {}));
}
function V() {
	return !0;
}
var H = {
	get(e, t, n) {
		return t === i ? n : e.get(t);
	},
	has(e, t) {
		return t === i || e.has(t);
	},
	set: V,
	deleteProperty: V,
	getOwnPropertyDescriptor(e, t) {
		return {
			configurable: !0,
			enumerable: !0,
			get() {
				return e.get(t);
			},
			set: V,
			deleteProperty: V
		};
	},
	ownKeys(e) {
		return e.keys();
	}
};
function U(e) {
	return (e = typeof e == "function" ? e() : e) ? e : {};
}
function be() {
	for (let e = 0, t = this.length; e < t; ++e) {
		let t = this[e]();
		if (t !== void 0) return t;
	}
}
function xe(...e) {
	let t = !1;
	for (let n = 0; n < e.length; n++) {
		let r = e[n];
		t ||= !!r && i in r, e[n] = typeof r == "function" ? (t = !0, S(r)) : r;
	}
	if (a && t) return new Proxy({
		get(t) {
			for (let n = e.length - 1; n >= 0; n--) {
				let r = U(e[n])[t];
				if (r !== void 0) return r;
			}
		},
		has(t) {
			for (let n = e.length - 1; n >= 0; n--) if (t in U(e[n])) return !0;
			return !1;
		},
		keys() {
			let t = [];
			for (let n = 0; n < e.length; n++) t.push(...Object.keys(U(e[n])));
			return [...new Set(t)];
		}
	}, H);
	let n = {}, r = Object.create(null);
	for (let t = e.length - 1; t >= 0; t--) {
		let i = e[t];
		if (!i) continue;
		let a = Object.getOwnPropertyNames(i);
		for (let e = a.length - 1; e >= 0; e--) {
			let t = a[e];
			if (t === "__proto__" || t === "constructor") continue;
			let o = Object.getOwnPropertyDescriptor(i, t);
			if (!r[t]) r[t] = o.get ? {
				enumerable: !0,
				configurable: !0,
				get: be.bind(n[t] = [o.get.bind(i)])
			} : o.value === void 0 ? void 0 : o;
			else {
				let e = n[t];
				e && (o.get ? e.push(o.get.bind(i)) : o.value !== void 0 && e.push(() => o.value));
			}
		}
	}
	let o = {}, s = Object.keys(r);
	for (let e = s.length - 1; e >= 0; e--) {
		let t = s[e], n = r[t];
		n && n.get ? Object.defineProperty(o, t, n) : o[t] = n ? n.value : void 0;
	}
	return o;
}
function Se(e, ...t) {
	let n = t.length;
	if (a && i in e) {
		let r = n > 1 ? t.flat() : t[0], i = t.map((t) => new Proxy({
			get(n) {
				return t.includes(n) ? e[n] : void 0;
			},
			has(n) {
				return t.includes(n) && n in e;
			},
			keys() {
				return t.filter((t) => t in e);
			}
		}, H));
		return i.push(new Proxy({
			get(t) {
				return r.includes(t) ? void 0 : e[t];
			},
			has(t) {
				return !r.includes(t) && t in e;
			},
			keys() {
				return Object.keys(e).filter((e) => !r.includes(e));
			}
		}, H)), i;
	}
	let r = [];
	for (let e = 0; e <= n; e++) r[e] = {};
	for (let i of Object.getOwnPropertyNames(e)) {
		let a = n;
		for (let e = 0; e < t.length; e++) if (t[e].includes(i)) {
			a = e;
			break;
		}
		let o = Object.getOwnPropertyDescriptor(e, i);
		!o.get && !o.set && o.enumerable && o.writable && o.configurable ? r[a][i] = o.value : Object.defineProperty(r[a], i, o);
	}
	return r;
}
var Ce = (e) => `Stale read from <${e}>.`;
function we(e) {
	let t = "fallback" in e && { fallback: () => e.fallback };
	return S(ve(() => e.each, e.children, t || void 0));
}
function Te(e) {
	let t = "fallback" in e && { fallback: () => e.fallback };
	return S(ye(() => e.each, e.children, t || void 0));
}
function Ee(e) {
	let t = e.keyed, n = S(() => e.when, void 0, void 0), r = t ? n : S(n, void 0, { equals: (e, t) => !e == !t });
	return S(() => {
		let i = r();
		if (i) {
			let a = e.children;
			return typeof a == "function" && a.length > 0 ? C(() => a(t ? i : () => {
				if (!C(r)) throw Ce("Show");
				return n();
			})) : a;
		}
		return e.fallback;
	}, void 0, void 0);
}
function De(e) {
	let t = se(() => e.children), n = S(() => {
		let e = t(), n = Array.isArray(e) ? e : [e], r = () => void 0;
		for (let e = 0; e < n.length; e++) {
			let t = e, i = n[e], a = r, o = S(() => a() ? void 0 : i.when, void 0, void 0), s = i.keyed ? o : S(o, void 0, { equals: (e, t) => !e == !t });
			r = () => a() || (s() ? [
				t,
				o,
				i
			] : void 0);
		}
		return r;
	});
	return S(() => {
		let t = n()();
		if (!t) return e.fallback;
		let [r, i, a] = t, o = a.children;
		return typeof o == "function" && o.length > 0 ? C(() => o(a.keyed ? i() : () => {
			if (C(n)()?.[0] !== r) throw Ce("Match");
			return i();
		})) : o;
	}, void 0, void 0);
}
function Oe(e) {
	return e;
}
var W;
function ke(t) {
	let n;
	e.context && e.load && (n = e.load(e.getContextId()));
	let [r, i] = b(n, void 0);
	return W ||= /* @__PURE__ */ new Set(), W.add(i), w(() => W.delete(i)), S(() => {
		let e;
		if (e = r()) {
			let n = t.fallback;
			return typeof n == "function" && n.length ? C(() => n(e, () => i())) : n;
		}
		return te(() => t.children, i);
	}, void 0, void 0);
}
var Ae = (e, t) => e.showContent === t.showContent && e.showFallback === t.showFallback, G = /* #__PURE__ */ oe();
function je(e) {
	let [t, n] = b(() => ({ inFallback: !1 })), r, i = E(G), [a, o] = b([]);
	i && (r = i.register(S(() => t()().inFallback)));
	let s = S((t) => {
		let n = e.revealOrder, i = e.tail, { showContent: o = !0, showFallback: s = !0 } = r ? r() : {}, c = a(), l = n === "backwards";
		if (n === "together") {
			let e = c.every((e) => !e()), t = c.map(() => ({
				showContent: e && o,
				showFallback: s
			}));
			return t.inFallback = !e, t;
		}
		let u = !1, d = t.inFallback, f = [];
		for (let e = 0, t = c.length; e < t; e++) {
			let n = l ? t - e - 1 : e, r = c[n]();
			if (!u && !r) f[n] = {
				showContent: o,
				showFallback: s
			};
			else {
				let e = !u;
				e && (d = !0), f[n] = {
					showContent: e,
					showFallback: !i || e && i === "collapsed" ? s : !1
				}, u = !0;
			}
		}
		return u || (d = !1), f.inFallback = d, f;
	}, { inFallback: !1 });
	return n(() => s), B(G.Provider, {
		value: { register: (e) => {
			let t;
			return o((n) => (t = n.length, [...n, e])), S(() => s()[t], void 0, { equals: Ae });
		} },
		get children() {
			return e.children;
		}
	});
}
function Me(t) {
	let r = 0, i, a, o, s, c, [l, u] = b(!1), d = ce(), f = {
		increment: () => {
			++r === 1 && u(!0);
		},
		decrement: () => {
			--r === 0 && u(!1);
		},
		inFallback: l,
		effects: [],
		resolved: !1
	}, p = T();
	if (e.context && e.load) {
		let t = e.getContextId(), r = e.load(t);
		if (r && (typeof r != "object" || r.s !== 1 ? o = r : e.gather(t)), o && o !== "$$f") {
			let [r, i] = b(void 0, { equals: !1 });
			s = r, o.then(() => {
				if (e.done) return i();
				e.gather(t), n(a), i(), n();
			}, (e) => {
				c = e, i();
			});
		}
	}
	let m = E(G);
	m && (i = m.register(f.inFallback));
	let h;
	return w(() => h && h()), B(d.Provider, {
		value: f,
		get children() {
			return S(() => {
				if (c) throw c;
				if (a = e.context, s) {
					s(), s = void 0;
					return;
				}
				a && o === "$$f" && n();
				let r = S(() => t.children);
				return S((e) => {
					let s = f.inFallback(), { showContent: c = !0, showFallback: l = !0 } = i ? i() : {};
					if ((!s || o && o !== "$$f") && c) return f.resolved = !0, h && h(), h = a = o = void 0, ae(f.effects), r();
					if (l) return h ? e : y((e) => (h = e, a &&= (n({
						id: a.id + "F",
						count: 0
					}), void 0), t.fallback), p);
				});
			});
		}
	});
}
//#endregion
//#region node_modules/solid-js/web/dist/web.js
var Ne = /*#__PURE__*/ new Set([
	"className",
	"value",
	"readOnly",
	"noValidate",
	"formNoValidate",
	"isMap",
	"noModule",
	"playsInline",
	"adAuctionHeaders",
	"allowFullscreen",
	"browsingTopics",
	"defaultChecked",
	"defaultMuted",
	"defaultSelected",
	"disablePictureInPicture",
	"disableRemotePlayback",
	"preservesPitch",
	"shadowRootClonable",
	"shadowRootCustomElementRegistry",
	"shadowRootDelegatesFocus",
	"shadowRootSerializable",
	"sharedStorageWritable",
	.../* @__PURE__ */ "allowfullscreen.async.alpha.autofocus.autoplay.checked.controls.default.disabled.formnovalidate.hidden.indeterminate.inert.ismap.loop.multiple.muted.nomodule.novalidate.open.playsinline.readonly.required.reversed.seamless.selected.adauctionheaders.browsingtopics.credentialless.defaultchecked.defaultmuted.defaultselected.defer.disablepictureinpicture.disableremoteplayback.preservespitch.shadowrootclonable.shadowrootcustomelementregistry.shadowrootdelegatesfocus.shadowrootserializable.sharedstoragewritable".split(".")
]), Pe = /*#__PURE__*/ new Set([
	"innerHTML",
	"textContent",
	"innerText",
	"children"
]), Fe = /*#__PURE__*/ Object.assign(Object.create(null), {
	className: "class",
	htmlFor: "for"
}), Ie = /*#__PURE__*/ Object.assign(Object.create(null), {
	class: "className",
	novalidate: {
		$: "noValidate",
		FORM: 1
	},
	formnovalidate: {
		$: "formNoValidate",
		BUTTON: 1,
		INPUT: 1
	},
	ismap: {
		$: "isMap",
		IMG: 1
	},
	nomodule: {
		$: "noModule",
		SCRIPT: 1
	},
	playsinline: {
		$: "playsInline",
		VIDEO: 1
	},
	readonly: {
		$: "readOnly",
		INPUT: 1,
		TEXTAREA: 1
	},
	adauctionheaders: {
		$: "adAuctionHeaders",
		IFRAME: 1
	},
	allowfullscreen: {
		$: "allowFullscreen",
		IFRAME: 1
	},
	browsingtopics: {
		$: "browsingTopics",
		IMG: 1
	},
	defaultchecked: {
		$: "defaultChecked",
		INPUT: 1
	},
	defaultmuted: {
		$: "defaultMuted",
		AUDIO: 1,
		VIDEO: 1
	},
	defaultselected: {
		$: "defaultSelected",
		OPTION: 1
	},
	disablepictureinpicture: {
		$: "disablePictureInPicture",
		VIDEO: 1
	},
	disableremoteplayback: {
		$: "disableRemotePlayback",
		AUDIO: 1,
		VIDEO: 1
	},
	preservespitch: {
		$: "preservesPitch",
		AUDIO: 1,
		VIDEO: 1
	},
	shadowrootclonable: {
		$: "shadowRootClonable",
		TEMPLATE: 1
	},
	shadowrootdelegatesfocus: {
		$: "shadowRootDelegatesFocus",
		TEMPLATE: 1
	},
	shadowrootserializable: {
		$: "shadowRootSerializable",
		TEMPLATE: 1
	},
	sharedstoragewritable: {
		$: "sharedStorageWritable",
		IFRAME: 1,
		IMG: 1
	}
});
function Le(e, t) {
	let n = Ie[e];
	return typeof n == "object" ? n[t] ? n.$ : void 0 : n;
}
var Re = /*#__PURE__*/ new Set([
	"beforeinput",
	"click",
	"dblclick",
	"contextmenu",
	"focusin",
	"focusout",
	"input",
	"keydown",
	"keyup",
	"mousedown",
	"mousemove",
	"mouseout",
	"mouseover",
	"mouseup",
	"pointerdown",
	"pointermove",
	"pointerout",
	"pointerover",
	"pointerup",
	"touchend",
	"touchmove",
	"touchstart"
]), ze = /*#__PURE__*/ new Set(/* @__PURE__ */ "altGlyph.altGlyphDef.altGlyphItem.animate.animateColor.animateMotion.animateTransform.circle.clipPath.color-profile.cursor.defs.desc.ellipse.feBlend.feColorMatrix.feComponentTransfer.feComposite.feConvolveMatrix.feDiffuseLighting.feDisplacementMap.feDistantLight.feDropShadow.feFlood.feFuncA.feFuncB.feFuncG.feFuncR.feGaussianBlur.feImage.feMerge.feMergeNode.feMorphology.feOffset.fePointLight.feSpecularLighting.feSpotLight.feTile.feTurbulence.filter.font.font-face.font-face-format.font-face-name.font-face-src.font-face-uri.foreignObject.g.glyph.glyphRef.hkern.image.line.linearGradient.marker.mask.metadata.missing-glyph.mpath.path.pattern.polygon.polyline.radialGradient.rect.set.stop.svg.switch.symbol.text.textPath.tref.tspan.use.view.vkern".split(".")), Be = {
	xlink: "http://www.w3.org/1999/xlink",
	xml: "http://www.w3.org/XML/1998/namespace"
}, Ve = /*#__PURE__*/ new Set(/* @__PURE__ */ "html.base.head.link.meta.style.title.body.address.article.aside.footer.header.main.nav.section.body.blockquote.dd.div.dl.dt.figcaption.figure.hr.li.ol.p.pre.ul.a.abbr.b.bdi.bdo.br.cite.code.data.dfn.em.i.kbd.mark.q.rp.rt.ruby.s.samp.small.span.strong.sub.sup.time.u.var.wbr.area.audio.img.map.track.video.embed.iframe.object.param.picture.portal.source.svg.math.canvas.noscript.script.del.ins.caption.col.colgroup.table.tbody.td.tfoot.th.thead.tr.button.datalist.fieldset.form.input.label.legend.meter.optgroup.option.output.progress.select.textarea.details.dialog.menu.summary.details.slot.template.acronym.applet.basefont.bgsound.big.blink.center.content.dir.font.frame.frameset.hgroup.image.keygen.marquee.menuitem.nobr.noembed.noframes.plaintext.rb.rtc.shadow.spacer.strike.tt.xmp.a.abbr.acronym.address.applet.area.article.aside.audio.b.base.basefont.bdi.bdo.bgsound.big.blink.blockquote.body.br.button.canvas.caption.center.cite.code.col.colgroup.content.data.datalist.dd.del.details.dfn.dialog.dir.div.dl.dt.em.embed.fieldset.figcaption.figure.font.footer.form.frame.frameset.head.header.hgroup.hr.html.i.iframe.image.img.input.ins.kbd.keygen.label.legend.li.link.main.map.mark.marquee.menu.menuitem.meta.meter.nav.nobr.noembed.noframes.noscript.object.ol.optgroup.option.output.p.param.picture.plaintext.portal.pre.progress.q.rb.rp.rt.rtc.ruby.s.samp.script.section.select.shadow.slot.small.source.spacer.span.strike.strong.style.sub.summary.sup.table.tbody.td.template.textarea.tfoot.th.thead.time.title.tr.track.tt.u.ul.var.video.wbr.xmp.input.h1.h2.h3.h4.h5.h6.webview.isindex.listing.multicol.nextid.noindex.search".split(".")), He = (e) => S(() => e());
function Ue(e, t, n) {
	let r = n.length, i = t.length, a = r, o = 0, s = 0, c = t[i - 1].nextSibling, l = null;
	for (; o < i || s < a;) {
		if (t[o] === n[s]) {
			o++, s++;
			continue;
		}
		for (; t[i - 1] === n[a - 1];) i--, a--;
		if (i === o) {
			let t = a < r ? s ? n[s - 1].nextSibling : n[a - s] : c;
			for (; s < a;) e.insertBefore(n[s++], t);
		} else if (a === s) for (; o < i;) (!l || !l.has(t[o])) && t[o].remove(), o++;
		else if (t[o] === n[a - 1] && n[s] === t[i - 1]) {
			let r = t[--i].nextSibling;
			e.insertBefore(n[s++], t[o++].nextSibling), e.insertBefore(n[--a], r), t[i] = n[a];
		} else {
			if (!l) {
				l = /* @__PURE__ */ new Map();
				let e = s;
				for (; e < a;) l.set(n[e], e++);
			}
			let r = l.get(t[o]);
			if (r != null) {
				if (s < r && r < a) {
					let c = o, u = 1, d;
					for (; ++c < i && c < a && (d = l.get(t[c])) != null && d === r + u;) u++;
					if (u > r - s) {
						let i = t[o];
						for (; s < r;) e.insertBefore(n[s++], i);
					} else e.replaceChild(n[s++], t[o++]);
				} else o++;
			} else t[o++].remove();
		}
	}
}
var K = "_$DX_DELEGATE";
function We(e, t, n, r = {}) {
	let i;
	return y((r) => {
		i = r, t === document ? e() : J(t, e(), t.firstChild ? null : void 0, n);
	}, r.owner), () => {
		i(), t.textContent = "";
	};
}
function Ge(e, t, n, r) {
	let i, a = () => {
		let t = r ? document.createElementNS("http://www.w3.org/1998/Math/MathML", "template") : document.createElement("template");
		return t.innerHTML = e, n ? t.content.firstChild.firstChild : r ? t.firstChild : t.content.firstChild;
	}, o = t ? () => C(() => document.importNode(i ||= a(), !0)) : () => (i ||= a()).cloneNode(!0);
	return o.cloneNode = o, o;
}
function Ke(e, t = window.document) {
	let n = t[K] || (t[K] = /* @__PURE__ */ new Set());
	for (let r = 0, i = e.length; r < i; r++) {
		let i = e[r];
		n.has(i) || (n.add(i), t.addEventListener(i, lt));
	}
}
function qe(e = window.document) {
	if (e[K]) {
		for (let t of e[K].keys()) e.removeEventListener(t, lt);
		delete e[K];
	}
}
function Je(e, t, n) {
	Y(e) || (e[t] = n);
}
function q(e, t, n) {
	Y(e) || (n == null ? e.removeAttribute(t) : e.setAttribute(t, n));
}
function Ye(e, t, n, r) {
	Y(e) || (r == null ? e.removeAttributeNS(t, n) : e.setAttributeNS(t, n, r));
}
function Xe(e, t, n) {
	Y(e) || (n ? e.setAttribute(t, "") : e.removeAttribute(t));
}
function Ze(e, t) {
	Y(e) || (t == null ? e.removeAttribute("class") : e.className = t);
}
function Qe(e, t, n, r) {
	if (r) Array.isArray(n) ? (e[`$$${t}`] = n[0], e[`$$${t}Data`] = n[1]) : e[`$$${t}`] = n;
	else if (Array.isArray(n)) {
		let r = n[0];
		e.addEventListener(t, n[0] = (t) => r.call(e, n[1], t));
	} else e.addEventListener(t, n, typeof n != "function" && n);
}
function $e(e, t, n = {}) {
	let r = Object.keys(t || {}), i = Object.keys(n), a, o;
	for (a = 0, o = i.length; a < o; a++) {
		let r = i[a];
		!r || r === "undefined" || t[r] || (X(e, r, !1), delete n[r]);
	}
	for (a = 0, o = r.length; a < o; a++) {
		let i = r[a], o = !!t[i];
		!i || i === "undefined" || n[i] === o || !o || (X(e, i, !0), n[i] = o);
	}
	return n;
}
function et(e, t, n) {
	if (!t) return n ? q(e, "style") : t;
	let r = e.style;
	if (typeof t == "string") return r.cssText = t;
	typeof n == "string" && (r.cssText = n = void 0), n ||= {}, t ||= {};
	let i, a;
	for (a in n) t[a] ?? r.removeProperty(a), delete n[a];
	for (a in t) i = t[a], i !== n[a] && (r.setProperty(a, i), n[a] = i);
	return n;
}
function tt(e, t, n) {
	n == null ? e.style.removeProperty(t) : e.style.setProperty(t, n);
}
function nt(e, t = {}, n, r) {
	let i = {};
	return r || x(() => i.children = Z(e, t.children, i.children)), x(() => typeof t.ref == "function" && it(t.ref, e)), x(() => at(e, t, n, !0, i, !0)), i;
}
function rt(e, t) {
	let n = e[t];
	return Object.defineProperty(e, t, {
		get() {
			return n();
		},
		enumerable: !0
	}), e;
}
function it(e, t, n) {
	return C(() => e(t, n));
}
function J(e, t, n, r) {
	if (n !== void 0 && !r && (r = []), typeof t != "function") return Z(e, t, r, n);
	x((r) => Z(e, t(), r, n), r);
}
function at(e, t, n, r, i = {}, a = !1) {
	t ||= {};
	for (let r in i) if (!(r in t)) {
		if (r === "children") continue;
		i[r] = ct(e, r, null, i[r], n, a, t);
	}
	for (let o in t) {
		if (o === "children") {
			r || Z(e, t.children);
			continue;
		}
		let s = t[o];
		i[o] = ct(e, o, s, i[o], n, a, t);
	}
}
function ot(t) {
	let n, r;
	return !Y() || !(n = e.registry.get(r = dt())) ? t() : (e.completed && e.completed.add(n), e.registry.delete(r), n);
}
function Y(t) {
	return !!e.context && !e.done && (!t || t.isConnected);
}
function st(e) {
	return e.toLowerCase().replace(/-([a-z])/g, (e, t) => t.toUpperCase());
}
function X(e, t, n) {
	let r = t.trim().split(/\s+/);
	for (let t = 0, i = r.length; t < i; t++) e.classList.toggle(r[t], n);
}
function ct(e, t, n, r, i, a, o) {
	let s, c, l, u, d;
	if (t === "style") return et(e, n, r);
	if (t === "classList") return $e(e, n, r);
	if (n === r) return r;
	if (t === "ref") a || n(e);
	else if (t.slice(0, 3) === "on:") {
		let i = t.slice(3);
		r && e.removeEventListener(i, r, typeof r != "function" && r), n && e.addEventListener(i, n, typeof n != "function" && n);
	} else if (t.slice(0, 10) === "oncapture:") {
		let i = t.slice(10);
		r && e.removeEventListener(i, r, !0), n && e.addEventListener(i, n, !0);
	} else if (t.slice(0, 2) === "on") {
		let i = t.slice(2).toLowerCase(), a = Re.has(i);
		if (!a && r) {
			let t = Array.isArray(r) ? r[0] : r;
			e.removeEventListener(i, t);
		}
		(a || n) && (Qe(e, i, n, a), a && Ke([i]));
	} else if (t.slice(0, 5) === "attr:") q(e, t.slice(5), n);
	else if (t.slice(0, 5) === "bool:") Xe(e, t.slice(5), n);
	else if ((d = t.slice(0, 5) === "prop:") || (l = Pe.has(t)) || !i && ((u = Le(t, e.tagName)) || (c = Ne.has(t))) || (s = e.nodeName.includes("-") || "is" in o)) {
		if (d) t = t.slice(5), c = !0;
		else if (Y(e)) return n;
		t === "class" || t === "className" ? Ze(e, n) : s && !c && !l ? e[st(t)] = n : e[u || t] = n;
	} else {
		let r = i && t.indexOf(":") > -1 && Be[t.split(":")[0]];
		r ? Ye(e, r, t, n) : q(e, Fe[t] || t, n);
	}
	return n;
}
function lt(t) {
	if (e.registry && e.events && e.events.find(([e, n]) => n === t)) return;
	let n = t.target, r = `$$${t.type}`, i = t.target, a = t.currentTarget, o = (e) => Object.defineProperty(t, "target", {
		configurable: !0,
		value: e
	}), s = () => {
		let e = n[r];
		if (e && !n.disabled) {
			let i = n[`${r}Data`];
			if (i === void 0 ? e.call(n, t) : e.call(n, i, t), t.cancelBubble) return;
		}
		return n.host && typeof n.host != "string" && !n.host._$host && n.contains(t.target) && o(n.host), !0;
	}, c = () => {
		for (; s() && (n = n._$host || n.parentNode || n.host););
	};
	if (Object.defineProperty(t, "currentTarget", {
		configurable: !0,
		get() {
			return n || document;
		}
	}), e.registry && !e.done && (e.done = _$HY.done = !0), t.composedPath) {
		let e = t.composedPath();
		o(e[0]);
		for (let t = 0; t < e.length - 2 && (n = e[t], s()); t++) {
			if (n._$host) {
				n = n._$host, c();
				break;
			}
			if (n.parentNode === a) break;
		}
	} else c();
	o(i);
}
function Z(e, t, n, r, i) {
	let a = Y(e);
	if (a) {
		!n && (n = [...e.childNodes]);
		let t = [];
		for (let e = 0; e < n.length; e++) {
			let r = n[e];
			r.nodeType === 8 && r.data.slice(0, 2) === "!$" ? r.remove() : t.push(r);
		}
		n = t;
	}
	for (; typeof n == "function";) n = n();
	if (t === n) return n;
	let o = typeof t, s = r !== void 0;
	if (e = s && n[0] && n[0].parentNode || e, o === "string" || o === "number") {
		if (a || o === "number" && (t = t.toString(), t === n)) return n;
		if (s) {
			let i = n[0];
			i && i.nodeType === 3 ? i.data !== t && (i.data = t) : i = document.createTextNode(t), n = $(e, n, r, i);
		} else n = n !== "" && typeof n == "string" ? e.firstChild.data = t : e.textContent = t;
	} else if (t == null || o === "boolean") {
		if (a) return n;
		n = $(e, n, r);
	} else if (o === "function") return x(() => {
		let i = t();
		for (; typeof i == "function";) i = i();
		n = Z(e, i, n, r);
	}), () => n;
	else if (Array.isArray(t)) {
		let o = [], c = n && Array.isArray(n);
		if (Q(o, t, n, i)) return x(() => n = Z(e, o, n, r, !0)), () => n;
		if (a) {
			if (!o.length) return n;
			if (r === void 0) return n = [...e.childNodes];
			let t = o[0];
			if (t.parentNode !== e) return n;
			let i = [t];
			for (; (t = t.nextSibling) !== r;) i.push(t);
			return n = i;
		}
		if (o.length === 0) {
			if (n = $(e, n, r), s) return n;
		} else c ? n.length === 0 ? ut(e, o, r) : Ue(e, n, o) : (n && $(e), ut(e, o));
		n = o;
	} else if (t.nodeType) {
		if (a && t.parentNode) return n = s ? [t] : t;
		if (Array.isArray(n)) {
			if (s) return n = $(e, n, r, t);
			$(e, n, null, t);
		} else n == null || n === "" || !e.firstChild ? e.appendChild(t) : e.replaceChild(t, e.firstChild);
		n = t;
	}
	return n;
}
function Q(e, t, n, r) {
	let i = !1;
	for (let a = 0, o = t.length; a < o; a++) {
		let o = t[a], s = n && n[e.length], c;
		if (o != null && o !== !0 && o !== !1) {
			if ((c = typeof o) == "object" && o.nodeType) e.push(o);
			else if (Array.isArray(o)) i = Q(e, o, s) || i;
			else if (c === "function") {
				if (r) {
					for (; typeof o == "function";) o = o();
					i = Q(e, Array.isArray(o) ? o : [o], Array.isArray(s) ? s : [s]) || i;
				} else e.push(o), i = !0;
			} else {
				let t = String(o);
				s && s.nodeType === 3 && s.data === t ? e.push(s) : e.push(document.createTextNode(t));
			}
		}
	}
	return i;
}
function ut(e, t, n = null) {
	for (let r = 0, i = t.length; r < i; r++) e.insertBefore(t[r], n);
}
function $(e, t, n, r) {
	if (n === void 0) return e.textContent = "";
	let i = r || document.createTextNode("");
	if (t.length) {
		let r = !1;
		for (let a = t.length - 1; a >= 0; a--) {
			let o = t[a];
			if (i !== o) {
				let t = o.parentNode === e;
				!r && !a ? t ? e.replaceChild(i, o) : e.insertBefore(i, n) : t && o.remove();
			} else r = !0;
		}
	} else e.insertBefore(i, n);
	return [i];
}
function dt() {
	return e.getNextContextId();
}
function ft(t, n) {
	!e.context && (t.innerHTML = n);
}
var pt = !1, mt = !1, ht = "http://www.w3.org/2000/svg";
function gt(e, t = !1, n = void 0) {
	return t ? document.createElementNS(ht, e) : document.createElement(e, { is: n });
}
function _t(t) {
	let { useShadow: n } = t, r = document.createTextNode(""), i = () => t.mount || document.body, a = T(), o, s = !!e.context;
	return ee(() => {
		s && (T().user = s = !1), o ||= ne(a, () => S(() => t.children));
		let e = i();
		if (e instanceof HTMLHeadElement) {
			let [t, n] = b(!1);
			y((n) => J(e, () => t() ? n() : o(), null)), w(() => n(!0));
		} else {
			let i = gt(t.isSVG ? "g" : "div", t.isSVG), a = n && i.attachShadow ? i.attachShadow({ mode: "open" }) : i;
			Object.defineProperty(i, "_$host", {
				get() {
					return r.parentNode;
				},
				configurable: !0
			}), J(a, o), e.appendChild(i), t.ref && t.ref(i), w(() => e.removeChild(i));
		}
	}, void 0, { render: !s }), r;
}
function vt(t, n) {
	let r = S(t);
	return S(() => {
		let t = r();
		switch (typeof t) {
			case "function": return C(() => t(n));
			case "string":
				let r = ze.has(t), i = e.context ? ot() : gt(t, r, C(() => n.is));
				return nt(i, n, r), i;
		}
	});
}
function yt(e) {
	let [, t] = Se(e, ["component"]);
	return vt(() => e.component, t);
}
//#endregion
export { Fe as Aliases, Pe as ChildProperties, Ve as DOMElements, Re as DelegatedEvents, yt as Dynamic, ke as ErrorBoundary, we as For, Te as Index, Oe as Match, _t as Portal, Ne as Properties, ze as SVGElements, Be as SVGNamespace, Ee as Show, Me as Suspense, je as SuspenseList, De as Switch, Qe as addEventListener, at as assign, $e as classList, Ze as className, qe as clearDelegatedEvents, B as createComponent, vt as createDynamic, Ke as delegateEvents, rt as dynamicProperty, x as effect, T as getOwner, Le as getPropAlias, ft as innerHTML, J as insert, mt as isDev, pt as isServer, He as memo, xe as mergeProps, We as render, q as setAttribute, Ye as setAttributeNS, Xe as setBoolAttribute, Je as setProperty, tt as setStyleProperty, nt as spread, et as style, Ge as template, C as untrack, it as use };
